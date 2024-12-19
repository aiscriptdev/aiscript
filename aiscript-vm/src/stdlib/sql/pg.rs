use gc_arena::{Gc, RefLock};
use sqlx::{Column, Row, TypeInfo, ValueRef};

use tokio::runtime::Handle;

use crate::{
    module::ModuleKind,
    object::Object,
    vm::{Context, State},
    NativeFn, Value, VmError,
};

// Create the PostgreSQL module with native functions
pub fn create_pg_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.sql.pg");

    let exports = [
        // Basic query function
        ("query", Value::NativeFunction(NativeFn(pg_query))),
        // Transaction function
        (
            "transaction",
            Value::NativeFunction(NativeFn(pg_transaction)),
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
            let mut query = sqlx::query(sql.to_str().unwrap());
            // Bind parameters directly with their native types
            for value in args.iter().skip(1) {
                match value {
                    Value::Number(n) => {
                        query = query.bind(n);
                    }
                    Value::String(s) => {
                        query = query.bind(s.to_str().unwrap());
                    }
                    Value::Boolean(b) => {
                        query = query.bind(b);
                    }
                    Value::Nil => {
                        query = query.bind(None::<&str>);
                    }
                    _ => {
                        return Err(VmError::RuntimeError(format!(
                            "Unsupported parameter: {}",
                            value
                        )))
                    }
                }
            }

            query
                .fetch_all(conn)
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

fn pg_transaction<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "transaction() requires a function argument.".into(),
        ));
    }

    // Get closure from args
    let closure = args[0].as_closure()?;
    let conn = state.pg_connection.as_ref().unwrap().clone();

    // Execute transaction in runtime
    Handle::current().block_on(async {
        let tx = conn
            .begin()
            .await
            .map_err(|e| VmError::RuntimeError(format!("Failed to begin transaction: {}", e)))?;

        // Call the closure
        let result = state.eval_function(closure.function, &[]);

        match result {
            Ok(_) => tx
                .commit()
                .await
                .map_err(|e| VmError::RuntimeError(format!("Failed to commit transaction: {}", e))),
            Err(e) => {
                tx.rollback().await.map_err(|e| {
                    VmError::RuntimeError(format!("Failed to rollback transaction: {}", e))
                })?;
                Err(e)
            }
        }
    })?;

    Ok(Value::Nil)
}
