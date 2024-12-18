use gc_arena::{Gc, RefLock};
use sqlx::{Column, Row};
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

    for (i, column) in row.columns().iter().enumerate() {
        let column_name = state.intern(column.name().as_bytes());

        let value = if let Ok(val) = row.try_get::<i64, _>(i) {
            Value::Number(val as f64)
        } else if let Ok(val) = row.try_get::<String, _>(i) {
            Value::String(state.intern(val.as_bytes()))
        } else if let Ok(val) = row.try_get::<bool, _>(i) {
            Value::Boolean(val)
        } else {
            Value::Nil
        };

        obj.fields.insert(column_name, value);
    }

    Value::Object(Gc::new(&state.get_context(), RefLock::new(obj)))
}

// Native function implementations
fn pg_query<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "query() requires at least a SQL query string.".into(),
        ));
    }

    let query = args[0].as_string()?;
    let conn = state.pg_connection.as_ref().unwrap();

    // Convert remaining args to parameters
    let params: Vec<sqlx::types::JsonValue> = args
        .iter()
        .skip(1)
        .map(|v| v.to_serde_value())
        .collect::<Vec<_>>();

    // Execute query in runtime
    let rows = Handle::current()
        .block_on(async {
            let query = sqlx::query(query.to_str().unwrap());

            // Bind parameters
            let query = params.iter().fold(query, |query, param| query.bind(param));

            query.fetch_all(&**conn).await
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

// fn _pg_query

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
