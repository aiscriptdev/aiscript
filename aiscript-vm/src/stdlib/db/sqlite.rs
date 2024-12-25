use std::{cell::RefCell, collections::HashMap};

use gc_arena::{Gc, GcRefLock, RefLock};
use sqlx::{Column, Row, Sqlite, TypeInfo, ValueRef};
use tokio::runtime::Handle;

use crate::{
    module::ModuleKind,
    object::{Class, Instance, Object},
    vm::{Context, State},
    NativeFn, Value, VmError,
};

thread_local! {
    static ACTIVE_TRANSACTION: RefCell<Option<sqlx::Transaction<'static, Sqlite>>> = const { RefCell::new(None) };
}

pub fn create_sqlite_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.db.sqlite");

    let exports = [
        ("query", Value::NativeFunction(NativeFn(sqlite_query))),
        ("query_as", Value::NativeFunction(NativeFn(sqlite_query_as))),
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

fn column_to_value<'gc>(
    ctx: Context<'gc>,
    row: &sqlx::sqlite::SqliteRow,
    i: usize,
    type_info: &sqlx::sqlite::SqliteTypeInfo,
) -> Result<Value<'gc>, VmError> {
    // Handle NULL values first
    if row.try_get_raw(i).map_or(true, |v| v.is_null()) {
        return Ok(Value::Nil);
    }

    let value = match type_info.name() {
        // Integer types
        "INTEGER" => row.try_get::<i64, _>(i).map(|v| Value::Number(v as f64)),

        // Floating-point types
        "REAL" => row.try_get::<f64, _>(i).map(Value::Number),

        // Text types
        "TEXT" => row
            .try_get::<String, _>(i)
            .map(|v| Value::String(ctx.intern(v.as_bytes()))),

        // Boolean type (stored as INTEGER in SQLite)
        "BOOLEAN" => row.try_get::<bool, _>(i).map(Value::Boolean),

        // Date/Time types (stored as TEXT or INTEGER in SQLite)
        "DATETIME" => row
            .try_get::<sqlx::types::chrono::NaiveDateTime, _>(i)
            .map(|v| Value::String(ctx.intern(v.to_string().as_bytes()))),

        // BLOB type
        "BLOB" => row
            .try_get::<Vec<u8>, _>(i)
            .map(|v| Value::String(ctx.intern(&v))),

        // JSON type (stored as TEXT in SQLite)
        "JSON" => row
            .try_get::<serde_json::Value, _>(i)
            .map(|v| Value::String(ctx.intern(v.to_string().as_bytes()))),

        // Default to string for unknown types
        _ => row
            .try_get::<String, _>(i)
            .map(|v| Value::String(ctx.intern(v.as_bytes()))),
    }
    .unwrap_or_else(|_| {
        // If conversion fails, try to get as string
        row.try_get::<String, _>(i)
            .map(|v| Value::String(ctx.intern(v.as_bytes())))
            .unwrap_or(Value::Nil)
    });
    Ok(value)
}

fn row_to_object<'gc>(ctx: Context<'gc>, row: &sqlx::sqlite::SqliteRow) -> Value<'gc> {
    let mut obj = Object::default();

    for (i, column) in row.columns().iter().enumerate() {
        let column_name = ctx.intern(column.name().as_bytes());
        let value = column_to_value(ctx, row, i, column.type_info()).unwrap_or(Value::Nil);
        obj.fields.insert(column_name, value);
    }

    Value::Object(Gc::new(&ctx, RefLock::new(obj)))
}

fn execute_query<'a, E>(
    executor: E,
    query: &str,
    bindings: Vec<Value<'_>>,
) -> Result<Vec<sqlx::sqlite::SqliteRow>, VmError>
where
    E: sqlx::Executor<'a, Database = sqlx::Sqlite>,
{
    Handle::current()
        .block_on(async {
            let mut query_builder = sqlx::query(query);

            // Bind parameters
            for value in bindings {
                match value {
                    Value::Number(n) => {
                        query_builder = query_builder.bind(n);
                    }
                    Value::String(s) => {
                        query_builder = query_builder.bind(s.to_str().unwrap());
                    }
                    Value::Boolean(b) => {
                        query_builder = query_builder.bind(b);
                    }
                    Value::Nil => {
                        query_builder = query_builder.bind(Option::<String>::None);
                    }
                    Value::Array(arr) => {
                        let arr = arr.borrow();
                        // SQLite doesn't have native array types, so convert to string representation
                        let json = serde_json::to_string(
                            &arr.iter().map(Value::to_serde_value).collect::<Vec<_>>(),
                        )
                        .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
                        query_builder = query_builder.bind(json);
                    }
                    _ => return Err(sqlx::Error::Protocol("Unsupported parameter type".into())),
                }
            }

            query_builder.fetch_all(executor).await
        })
        .map_err(|e| VmError::RuntimeError(format!("Database query error: {}", e)))
}

fn execute_typed_query<'gc, 'a, E>(
    ctx: Context<'gc>,
    executor: E,
    class: GcRefLock<'gc, Class<'gc>>,
    query: &str,
    bindings: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError>
where
    E: sqlx::Executor<'a, Database = sqlx::Sqlite>,
{
    // Execute the query
    let rows = execute_query(executor, query, bindings)?;

    // Convert rows to class instances
    let mut results = Vec::new();
    for row in rows {
        // Create new instance
        let mut instance = Instance::new(class);

        // Set fields from row data
        for (i, column) in row.columns().iter().enumerate() {
            let field_name = ctx.intern(column.name().as_bytes());
            let value = column_to_value(ctx, &row, i, column.type_info())?;
            instance.fields.insert(field_name, value);
        }

        results.push(Value::Instance(Gc::new(&ctx, RefLock::new(instance))));
    }

    Ok(Value::Array(Gc::new(&ctx, RefLock::new(results))))
}

fn sqlite_query<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "query() requires at least a SQL query string.".into(),
        ));
    }

    let sql = args[0].as_string()?;
    let ctx = state.get_context();
    let conn = state.sqlite_connection.as_ref().unwrap();

    let rows = execute_query(
        conn,
        sql.to_str().unwrap(),
        args.into_iter().skip(1).collect(),
    )?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row_to_object(ctx, &row));
    }

    Ok(Value::Array(Gc::new(&ctx, RefLock::new(results))))
}

fn sqlite_query_as<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() < 2 {
        return Err(VmError::RuntimeError(
            "query_as() requires a class and SQL query string.".into(),
        ));
    }

    let class = match args[0] {
        Value::Class(class) => class,
        _ => {
            return Err(VmError::RuntimeError(
                "First argument to query_as() must be a class.".into(),
            ))
        }
    };

    let sql = args[1].as_string()?;
    let ctx = state.get_context();
    let conn = state.sqlite_connection.as_ref().unwrap();

    execute_typed_query(
        ctx,
        conn,
        class,
        sql.to_str().unwrap(),
        args.into_iter().skip(2).collect(),
    )
}

mod transaction {
    use super::*;

    fn create_transaction_class(ctx: Context) -> Gc<RefLock<Class>> {
        let methods = [
            (ctx.intern(b"query"), Value::NativeFunction(NativeFn(query))),
            (
                ctx.intern(b"query_as"),
                Value::NativeFunction(NativeFn(query_as)),
            ),
            (
                ctx.intern(b"commit"),
                Value::NativeFunction(NativeFn(commit)),
            ),
            (
                ctx.intern(b"rollback"),
                Value::NativeFunction(NativeFn(rollback)),
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

    pub(super) fn begin_transaction<'gc>(
        state: &mut State<'gc>,
        _args: Vec<Value<'gc>>,
    ) -> Result<Value<'gc>, VmError> {
        let has_active = ACTIVE_TRANSACTION.with(|tx| tx.borrow().is_some());
        if has_active {
            return Err(VmError::RuntimeError("Transaction already active".into()));
        }

        let ctx = state.get_context();
        let conn = state.sqlite_connection.as_ref().unwrap();
        let tx = Handle::current()
            .block_on(async move { conn.begin().await })
            .map_err(|e| VmError::RuntimeError(format!("Failed to begin transaction: {}", e)))?;

        ACTIVE_TRANSACTION.with(|cell| {
            *cell.borrow_mut() = Some(tx);
        });

        let instance = Instance::new(create_transaction_class(ctx));
        Ok(Value::Instance(Gc::new(&ctx, RefLock::new(instance))))
    }

    fn query<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
        if args.is_empty() {
            return Err(VmError::RuntimeError(
                "query() requires a SQL query string.".into(),
            ));
        }

        let query = args[0].as_string()?;
        let ctx = state.get_context();

        let result = ACTIVE_TRANSACTION.with(|cell| {
            if let Some(tx) = (*cell.borrow_mut()).as_mut() {
                let rows = execute_query(
                    &mut **tx,
                    query.to_str().unwrap(),
                    args.into_iter().skip(1).collect(),
                );
                Some(rows)
            } else {
                None
            }
        });

        match result {
            Some(Ok(rows)) => {
                let mut results = Vec::new();
                for row in rows {
                    results.push(row_to_object(ctx, &row));
                }
                Ok(Value::Array(Gc::new(&ctx, RefLock::new(results))))
            }
            Some(Err(e)) => Err(VmError::RuntimeError(format!("Database error: {e}"))),
            None => Err(VmError::RuntimeError("No active transaction".into())),
        }
    }

    fn query_as<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
        if args.len() < 2 {
            return Err(VmError::RuntimeError(
                "query_as() requires a class and SQL query string.".into(),
            ));
        }

        let class = match args[0] {
            Value::Class(class) => class,
            _ => {
                return Err(VmError::RuntimeError(
                    "First argument to query_as() must be a class.".into(),
                ))
            }
        };

        let query = args[1].as_string()?;
        let ctx = state.get_context();

        let result = ACTIVE_TRANSACTION.with(|cell| {
            if let Some(tx) = (*cell.borrow_mut()).as_mut() {
                let bindings = args.into_iter().skip(2).collect();
                Some(execute_typed_query(
                    ctx,
                    &mut **tx,
                    class,
                    query.to_str().unwrap(),
                    bindings,
                ))
            } else {
                None
            }
        });

        match result {
            Some(result) => result,
            None => Err(VmError::RuntimeError("No active transaction".into())),
        }
    }

    fn commit<'gc>(_state: &mut State<'gc>, _args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
        let result = ACTIVE_TRANSACTION.with(|cell| {
            cell.borrow_mut()
                .take()
                .map(|tx| Handle::current().block_on(async { tx.commit().await }))
        });

        match result {
            Some(Ok(())) => Ok(Value::Nil),
            Some(Err(e)) => Err(VmError::RuntimeError(format!(
                "Failed to commit transaction: {e}"
            ))),
            None => Err(VmError::RuntimeError("No active transaction".into())),
        }
    }

    fn rollback<'gc>(
        _state: &mut State<'gc>,
        _args: Vec<Value<'gc>>,
    ) -> Result<Value<'gc>, VmError> {
        let result = ACTIVE_TRANSACTION.with(|cell| {
            cell.borrow_mut()
                .take()
                .map(|tx| Handle::current().block_on(async { tx.rollback().await }))
        });

        match result {
            Some(Ok(())) => Ok(Value::Nil),
            Some(Err(e)) => Err(VmError::RuntimeError(format!(
                "Failed to rollback transaction: {e}"
            ))),
            None => Err(VmError::RuntimeError("No active transaction".into())),
        }
    }
}
