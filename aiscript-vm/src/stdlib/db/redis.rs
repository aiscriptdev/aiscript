use std::cell::RefCell;

use gc_arena::{Gc, RefLock};
use redis::{RedisResult, Value as RedisValue};
use tokio::runtime::Handle;

use crate::{
    module::ModuleKind,
    object::Object,
    vm::{Context, State},
    NativeFn, Value, VmError,
};

thread_local! {
    static ACTIVE_TRANSACTION: RefCell<Option<redis::Pipeline>> = const { RefCell::new(None) };
}

// Create the Redis module with native functions
pub fn create_redis_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.db.redis");

    let exports = [
        ("cmd", Value::NativeFunction(NativeFn(redis_cmd))),
        ("pipeline", Value::NativeFunction(NativeFn(redis_pipeline))),
        ("exec", Value::NativeFunction(NativeFn(redis_exec))),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect();

    ModuleKind::Native { name, exports }
}

// Convert Redis value to AIScript value
fn redis_to_value(ctx: Context, value: RedisValue) -> Value {
    match value {
        RedisValue::Nil => Value::Nil,
        RedisValue::Int(i) => {
            // Check if it might be a boolean in disguise (from OK response)
            if i == 0 || i == 1 {
                Value::Boolean(i == 1)
            } else {
                Value::Number(i as f64)
            }
        }
        RedisValue::Double(d) => Value::Number(d),
        RedisValue::BulkString(bytes) => {
            if let Ok(s) = String::from_utf8(bytes.clone()) {
                Value::String(ctx.intern(s.as_bytes()))
            } else {
                Value::String(ctx.intern(&bytes))
            }
        }
        RedisValue::Array(values) => {
            let elements = values.into_iter().map(|v| redis_to_value(ctx, v)).collect();
            Value::Array(Gc::new(&ctx, RefLock::new(elements)))
        }
        RedisValue::SimpleString(s) => Value::String(ctx.intern(s.as_bytes())),
        RedisValue::Boolean(b) => Value::Boolean(b),
        RedisValue::Map(pairs) => {
            let mut obj = Object::default();
            for (key, val) in pairs {
                let key_str = match key {
                    RedisValue::SimpleString(s) => s,
                    RedisValue::BulkString(bytes) => {
                        if let Ok(s) = String::from_utf8(bytes) {
                            s
                        } else {
                            continue;
                        }
                    }
                    _ => format!("{:?}", key),
                };
                obj.fields
                    .insert(ctx.intern(key_str.as_bytes()), redis_to_value(ctx, val));
            }
            Value::Object(Gc::new(&ctx, RefLock::new(obj)))
        }
        RedisValue::Set(values) => {
            let elements = values.into_iter().map(|v| redis_to_value(ctx, v)).collect();
            Value::Array(Gc::new(&ctx, RefLock::new(elements)))
        }
        _ => Value::Nil,
    }
}

// Helper to convert AIScript value to Redis value
fn value_to_redis(value: &Value) -> RedisValue {
    match value {
        Value::Number(n) => {
            if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                RedisValue::Int(*n as i64)
            } else {
                RedisValue::Double(*n)
            }
        }
        Value::String(s) => RedisValue::BulkString(s.as_bytes().to_vec()),
        Value::Boolean(b) => RedisValue::Boolean(*b),
        Value::Array(arr) => {
            let values: Vec<RedisValue> = arr.borrow().iter().map(value_to_redis).collect();
            RedisValue::Array(values)
        }
        Value::Object(obj) => {
            let mut pairs = Vec::new();
            for (key, value) in &obj.borrow().fields {
                pairs.push((
                    RedisValue::BulkString(key.as_bytes().to_vec()),
                    value_to_redis(value),
                ));
            }
            RedisValue::Map(pairs)
        }
        Value::Nil => RedisValue::Nil,
        _ => RedisValue::SimpleString(format!("{}", value)),
    }
}

// Parse Redis command string and arguments
fn parse_redis_cmd(cmd_str: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    if parts.is_empty() {
        return (String::new(), Vec::new());
    }

    let command = parts[0].to_uppercase();
    let args = parts[1..].iter().map(|s| s.to_string()).collect();

    (command, args)
}

// Main command execution function
fn redis_cmd<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "cmd() requires a command string".into(),
        ));
    }

    let cmd_str = args[0].as_string()?.to_str().unwrap();
    let (command, mut redis_args) = parse_redis_cmd(cmd_str);
    if command.is_empty() {
        return Err(VmError::RuntimeError("Empty command string".into()));
    }

    // Add any additional arguments passed as values
    for arg in args.iter().skip(1) {
        redis_args.push(format!("{:?}", value_to_redis(arg)));
    }

    let ctx = state.get_context();
    let conn = state.redis_connection.as_mut().unwrap();

    // Execute the command
    let result: RedisResult<RedisValue> = Handle::current().block_on(async {
        let mut cmd = redis::cmd(&command);
        for arg in redis_args {
            cmd.arg(arg);
        }
        cmd.query_async(conn).await
    });

    match result {
        Ok(value) => Ok(redis_to_value(ctx, value)),
        Err(e) => Err(VmError::RuntimeError(format!("Redis error: {}", e))),
    }
}

// Start a pipeline
fn redis_pipeline<'gc>(
    _state: &mut State<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let has_active = ACTIVE_TRANSACTION.with(|tx| tx.borrow().is_some());
    if has_active {
        return Err(VmError::RuntimeError("Pipeline already active".into()));
    }

    // Create new pipeline
    let pipeline = redis::pipe();
    ACTIVE_TRANSACTION.with(|cell| {
        *cell.borrow_mut() = Some(pipeline);
    });

    Ok(Value::Boolean(true))
}

// Execute the pipeline
fn redis_exec<'gc>(state: &mut State<'gc>, _args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let ctx = state.get_context();
    let conn = state.redis_connection.as_mut().unwrap();

    let result: std::option::Option<RedisResult<Vec<RedisValue>>> =
        ACTIVE_TRANSACTION.with(|cell| {
            cell.borrow_mut().take().map(|mut pipeline| {
                Handle::current().block_on(async {
                    // Convert pipeline to vec of values using atomic()
                    pipeline.atomic().query_async(conn).await
                })
            })
        });

    match result {
        Some(Ok(values)) => {
            let elements = values.into_iter().map(|v| redis_to_value(ctx, v)).collect();
            Ok(Value::Array(Gc::new(&ctx, RefLock::new(elements))))
        }
        Some(Err(e)) => Err(VmError::RuntimeError(format!("Redis error: {}", e))),
        None => Err(VmError::RuntimeError("No active pipeline".into())),
    }
}
