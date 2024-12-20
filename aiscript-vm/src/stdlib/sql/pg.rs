use std::{cell::RefCell, collections::HashMap};

use gc_arena::{Gc, RefLock};
use sqlx::{Column, Postgres, Row, TypeInfo, ValueRef};

use tokio::runtime::Handle;

use crate::{
    module::ModuleKind,
    object::{Class, Instance, Object},
    vm::{Context, State},
    NativeFn, Value, VmError,
};

thread_local! {
    static ACTIVE_TRANSACTION: RefCell<Option<sqlx::Transaction<'static, Postgres>>> = const { RefCell::new(None) };
}

fn create_transaction_class(ctx: Context) -> Gc<RefLock<Class>> {
    let methods = [
        (
            ctx.intern(b"query"),
            Value::NativeFunction(NativeFn(transaction::query)),
        ),
        (
            ctx.intern(b"commit"),
            Value::NativeFunction(NativeFn(transaction::commit)),
        ),
        (
            ctx.intern(b"rollback"),
            Value::NativeFunction(NativeFn(transaction::rollback)),
        ),
    ]
    .into_iter()
    .collect();
    Gc::new(
        &ctx,
        RefLock::new(Class {
            name: ctx.intern(b"Transaction"),
            methods,
            static_methods: HashMap::default(),
        }),
    )
}

// Create the PostgreSQL module with native functions
pub fn create_pg_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.sql.pg");

    let exports = [
        ("query", Value::NativeFunction(NativeFn(pg_query))),
        (
            "begin_transaction",
            Value::NativeFunction(NativeFn(transaction::begin_transaction)),
        ),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect();

    ModuleKind::Native { name, exports }
}

// Convert database row to AIScript object
fn row_to_object<'gc>(state: &mut State<'gc>, row: &sqlx::postgres::PgRow) -> Value<'gc> {
    let mut obj = Object::default();
    let ctx = state.get_context();

    for (i, column) in row.columns().iter().enumerate() {
        let column_name = state.intern(column.name().as_bytes());
        let type_info = column.type_info();

        // Handle NULL values first
        if row.try_get_raw(i).map_or(true, |v| v.is_null()) {
            obj.fields.insert(column_name, Value::Nil);
            continue;
        }

        let value = match type_info.name() {
            // Integer types
            "INT2" | "SMALLINT" => row.try_get::<i16, _>(i).map(|v| Value::Number(v as f64)),
            "INT4" | "INTEGER" => row.try_get::<i32, _>(i).map(|v| Value::Number(v as f64)),
            "INT8" | "BIGINT" => row.try_get::<i64, _>(i).map(|v| Value::Number(v as f64)),

            // Serial types (same as integer types)
            "SERIAL2" | "SMALLSERIAL" => row.try_get::<i16, _>(i).map(|v| Value::Number(v as f64)),
            "SERIAL4" | "SERIAL" => row.try_get::<i32, _>(i).map(|v| Value::Number(v as f64)),
            "SERIAL8" | "BIGSERIAL" => row.try_get::<i64, _>(i).map(|v| Value::Number(v as f64)),

            // Floating-point types
            "FLOAT4" | "REAL" => row.try_get::<f32, _>(i).map(|v| Value::Number(v as f64)),
            "FLOAT8" | "DOUBLE PRECISION" => row.try_get::<f64, _>(i).map(Value::Number),

            // Decimal/numeric types
            // "NUMERIC" | "DECIMAL" => row
            //     .try_get::<sqlx::types::Decimal, _>(i)
            //     .map(|v| Value::Number(v.to_string().parse::<f64>().unwrap_or(0.0))),

            // Character types
            "VARCHAR" | "CHAR" | "TEXT" | "BPCHAR" | "NAME" => row
                .try_get::<String, _>(i)
                .map(|v| Value::String(state.intern(v.as_bytes()))),

            // Boolean type
            "BOOL" | "BOOLEAN" => row.try_get::<bool, _>(i).map(Value::Boolean),

            // UUID type
            "UUID" => row
                .try_get::<sqlx::types::Uuid, _>(i)
                .map(|v| Value::String(state.intern(v.to_string().as_bytes()))),

            // Date/Time types
            "DATE" => row
                .try_get::<sqlx::types::chrono::NaiveDate, _>(i)
                .map(|v| Value::String(state.intern(v.to_string().as_bytes()))),
            "TIME" => row
                .try_get::<sqlx::types::chrono::NaiveTime, _>(i)
                .map(|v| Value::String(state.intern(v.to_string().as_bytes()))),
            "TIMESTAMP" => row
                .try_get::<sqlx::types::chrono::NaiveDateTime, _>(i)
                .map(|v| Value::String(state.intern(v.to_string().as_bytes()))),
            "TIMESTAMPTZ" => row
                .try_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>(i)
                .map(|v| Value::String(state.intern(v.to_string().as_bytes()))),

            // JSON types
            "JSON" | "JSONB" => row
                .try_get::<serde_json::Value, _>(i)
                .map(|v| Value::String(state.intern(v.to_string().as_bytes()))),

            // Array types
            t if t.starts_with("_") => {
                match &t[1..] {
                    // Integer arrays
                    "INT2" | "SMALLINT" => row.try_get::<Vec<i16>, _>(i).map(|v| {
                        Value::Array(Gc::new(
                            &ctx,
                            RefLock::new(v.into_iter().map(|n| Value::Number(n as f64)).collect()),
                        ))
                    }),
                    "INT4" | "INTEGER" => row.try_get::<Vec<i32>, _>(i).map(|v| {
                        Value::Array(Gc::new(
                            &ctx,
                            RefLock::new(v.into_iter().map(|n| Value::Number(n as f64)).collect()),
                        ))
                    }),
                    "INT8" | "BIGINT" => row.try_get::<Vec<i64>, _>(i).map(|v| {
                        Value::Array(Gc::new(
                            &ctx,
                            RefLock::new(v.into_iter().map(|n| Value::Number(n as f64)).collect()),
                        ))
                    }),

                    // Float arrays
                    "FLOAT4" | "REAL" => row.try_get::<Vec<f32>, _>(i).map(|v| {
                        Value::Array(Gc::new(
                            &ctx,
                            RefLock::new(v.into_iter().map(|n| Value::Number(n as f64)).collect()),
                        ))
                    }),
                    "FLOAT8" | "DOUBLE PRECISION" => row.try_get::<Vec<f64>, _>(i).map(|v| {
                        Value::Array(Gc::new(
                            &ctx,
                            RefLock::new(v.into_iter().map(Value::Number).collect()),
                        ))
                    }),

                    // Text arrays
                    "VARCHAR" | "TEXT" => row.try_get::<Vec<String>, _>(i).map(|v| {
                        Value::Array(Gc::new(
                            &ctx,
                            RefLock::new(
                                v.into_iter()
                                    .map(|s| Value::String(state.intern(s.as_bytes())))
                                    .collect(),
                            ),
                        ))
                    }),

                    // Boolean arrays
                    "BOOL" | "BOOLEAN" => row.try_get::<Vec<bool>, _>(i).map(|v| {
                        Value::Array(Gc::new(
                            &ctx,
                            RefLock::new(v.into_iter().map(Value::Boolean).collect()),
                        ))
                    }),

                    // Default to string representation for unknown array types
                    _ => row.try_get::<Vec<String>, _>(i).map(|v| {
                        Value::Array(Gc::new(
                            &ctx,
                            RefLock::new(
                                v.into_iter()
                                    .map(|s| Value::String(state.intern(s.as_bytes())))
                                    .collect(),
                            ),
                        ))
                    }),
                }
            }

            // Binary data
            "BYTEA" => row
                .try_get::<Vec<u8>, _>(i)
                .map(|v| Value::String(state.intern(&v))),

            // Default to string for unknown types
            _ => row
                .try_get::<String, _>(i)
                .map(|v| Value::String(state.intern(v.as_bytes()))),
        }
        .unwrap_or_else(|_| {
            // If conversion fails, try to get as string
            row.try_get::<String, _>(i)
                .map(|v| Value::String(state.intern(v.as_bytes())))
                .unwrap_or(Value::Nil)
        });

        obj.fields.insert(column_name, value);
    }

    Value::Object(Gc::new(&ctx, RefLock::new(obj)))
}

async fn execute_query<'a, E>(
    executor: E,
    query: &str,
    bindings: Vec<Value<'_>>,
) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error>
where
    E: sqlx::Executor<'a, Database = sqlx::Postgres>,
{
    let mut query_builder = sqlx::query(query);

    // Bind parameters
    for value in bindings {
        match value {
            Value::Number(n) => {
                query_builder = query_builder.bind(n);
            }
            Value::String(s) => {
                let s_str = s.to_str().unwrap();
                // Try to parse special types from string
                if let Ok(uuid) = sqlx::types::Uuid::parse_str(s_str) {
                    query_builder = query_builder.bind(uuid);
                } else if let Ok(date) =
                    sqlx::types::chrono::NaiveDate::parse_from_str(s_str, "%Y-%m-%d")
                {
                    query_builder = query_builder.bind(date);
                } else if let Ok(datetime) =
                    sqlx::types::chrono::NaiveDateTime::parse_from_str(s_str, "%Y-%m-%dT%H:%M:%S")
                {
                    query_builder = query_builder.bind(datetime);
                } else {
                    query_builder = query_builder.bind(s_str);
                }
            }
            Value::Boolean(b) => {
                query_builder = query_builder.bind(b);
            }
            Value::Nil => {
                query_builder = query_builder.bind(Option::<String>::None);
            }
            Value::Array(arr) => {
                let arr = arr.borrow();
                if let Some(first) = arr.first() {
                    match first {
                        Value::Number(_) => {
                            let nums: Vec<f64> = arr
                                .iter()
                                .filter_map(|v| match v {
                                    Value::Number(n) => Some(*n),
                                    _ => None,
                                })
                                .collect();
                            query_builder = query_builder.bind(nums);
                        }
                        Value::String(_) => {
                            let strings: Vec<String> = arr
                                .iter()
                                .filter_map(|v| match v {
                                    Value::String(s) => Some(s.to_str().unwrap().to_string()),
                                    _ => None,
                                })
                                .collect();
                            query_builder = query_builder.bind(strings);
                        }
                        Value::Boolean(_) => {
                            let bools: Vec<bool> = arr
                                .iter()
                                .filter_map(|v| match v {
                                    Value::Boolean(b) => Some(*b),
                                    _ => None,
                                })
                                .collect();
                            query_builder = query_builder.bind(bools);
                        }
                        _ => {
                            return Err(sqlx::Error::Protocol(
                                "Unsupported array element type".into(),
                            ))
                        }
                    }
                } else {
                    query_builder = query_builder.bind::<Vec<String>>(vec![]);
                }
            }
            _ => return Err(sqlx::Error::Protocol("Unsupported parameter type".into())),
        }
    }

    query_builder.fetch_all(executor).await
}

// Native function implementations
fn pg_query<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "query() requires at least a SQL query string.".into(),
        ));
    }

    let sql = args[0].as_string()?;
    let conn = state.pg_connection.as_ref().unwrap();

    // Execute query in runtime
    let rows = Handle::current()
        .block_on(async {
            execute_query(
                conn,
                sql.to_str().unwrap(),
                args.into_iter().skip(1).collect(),
            )
            .await
            .map_err(|err| VmError::RuntimeError(err.to_string()))
        })
        .map_err(|e| VmError::RuntimeError(format!("Database error: {}", e)))?;

    // Convert rows to array of objects
    let mut results = Vec::new();
    for row in rows {
        results.push(row_to_object(state, &row));
    }

    Ok(Value::Array(Gc::new(
        &state.get_context(),
        RefLock::new(results),
    )))
}

mod transaction {
    use super::*;

    pub(super) fn begin_transaction<'gc>(
        state: &mut State<'gc>,
        _args: Vec<Value<'gc>>,
    ) -> Result<Value<'gc>, VmError> {
        // Check if there's already an active transaction
        let has_active = ACTIVE_TRANSACTION.with(|tx| tx.borrow().is_some());
        if has_active {
            return Err(VmError::RuntimeError("Transaction already active".into()));
        }

        let ctx = state.get_context();
        let conn = state.pg_connection.as_ref().unwrap();
        let tx = Handle::current()
            .block_on(async move { conn.begin().await })
            .map_err(|e| VmError::RuntimeError(format!("Failed to begin transaction: {}", e)))?;

        // Store transaction in thread local
        ACTIVE_TRANSACTION.with(|cell| {
            *cell.borrow_mut() = Some(tx);
        });

        // Create and return new instance
        let instance = Instance::new(create_transaction_class(ctx));
        Ok(Value::Instance(Gc::new(&ctx, RefLock::new(instance))))
    }

    pub(super) fn query<'gc>(
        state: &mut State<'gc>,
        args: Vec<Value<'gc>>,
    ) -> Result<Value<'gc>, VmError> {
        if args.is_empty() {
            return Err(VmError::RuntimeError(
                "query() requires a SQL query string.".into(),
            ));
        }

        let query = args[0].as_string()?;
        let ctx = state.get_context();

        // Execute query with the active transaction
        let result = ACTIVE_TRANSACTION.with(|cell| {
            if let Some(tx) = (*cell.borrow_mut()).as_mut() {
                let rows = Handle::current()
                    .block_on(async {
                        execute_query(&mut **tx, query.to_str().unwrap(), args[1..].to_vec()).await
                    })
                    .map_err(|e| VmError::RuntimeError(format!("Database error: {}", e)));

                Some(rows)
            } else {
                None
            }
        });

        match result {
            Some(Ok(rows)) => {
                // Convert rows to array of objects
                let mut results = Vec::new();
                for row in rows {
                    results.push(row_to_object(state, &row));
                }
                Ok(Value::Array(Gc::new(&ctx, RefLock::new(results))))
            }
            Some(Err(e)) => Err(VmError::RuntimeError(format!("Database error: {}", e))),
            None => Err(VmError::RuntimeError("No active transaction".into())),
        }
    }

    pub(super) fn commit<'gc>(
        _state: &mut State<'gc>,
        _args: Vec<Value<'gc>>,
    ) -> Result<Value<'gc>, VmError> {
        let result = ACTIVE_TRANSACTION.with(|cell| {
            cell.borrow_mut()
                .take() // Set ACTIVE_TRANSACTION to None
                .map(|tx| Handle::current().block_on(async { tx.commit().await }))
        });

        match result {
            Some(Ok(())) => Ok(Value::Nil),
            Some(Err(e)) => Err(VmError::RuntimeError(format!(
                "Failed to commit transaction: {}",
                e
            ))),
            None => Err(VmError::RuntimeError("No active transaction".into())),
        }
    }

    pub(super) fn rollback<'gc>(
        _state: &mut State<'gc>,
        _args: Vec<Value<'gc>>,
    ) -> Result<Value<'gc>, VmError> {
        let result = ACTIVE_TRANSACTION.with(|cell| {
            cell.borrow_mut()
                .take() // Set ACTIVE_TRANSACTION to None
                .map(|tx| Handle::current().block_on(async { tx.rollback().await }))
        });

        match result {
            Some(Ok(())) => Ok(Value::Nil),
            Some(Err(e)) => Err(VmError::RuntimeError(format!(
                "Failed to rollback transaction: {}",
                e
            ))),
            None => Err(VmError::RuntimeError("No active transaction".into())),
        }
    }
}
