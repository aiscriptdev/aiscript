use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use aiscript_arena::Gc;
use std::{
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    float_arg,
    module::ModuleKind,
    string_arg,
    vm::{Context, State},
    NativeFn, Value, VmError,
};

pub fn create_time_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern_static("std.time");

    let exports = [
        // Constants
        ("UNIX_EPOCH", Value::Number(0.0)),
        // Core time functions
        ("now", Value::NativeFunction(NativeFn(time_now))),
        (
            "unix_timestamp",
            Value::NativeFunction(NativeFn(time_unix_timestamp)),
        ),
        ("sleep", Value::NativeFunction(NativeFn(time_sleep))),
        // Time conversion functions
        ("to_utc", Value::NativeFunction(NativeFn(time_to_utc))),
        ("to_local", Value::NativeFunction(NativeFn(time_to_local))),
        (
            "format_datetime",
            Value::NativeFunction(NativeFn(time_format_datetime)),
        ),
        (
            "parse_datetime",
            Value::NativeFunction(NativeFn(time_parse_datetime)),
        ),
        // Duration functions
        ("seconds", Value::NativeFunction(NativeFn(time_seconds))),
        ("minutes", Value::NativeFunction(NativeFn(time_minutes))),
        ("hours", Value::NativeFunction(NativeFn(time_hours))),
        ("days", Value::NativeFunction(NativeFn(time_days))),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect();

    ModuleKind::Native { name, exports }
}

/// Returns the current time as Unix timestamp in seconds with fractional part
fn time_now<'gc>(_state: &mut State<'gc>, _args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| VmError::RuntimeError(format!("Failed to get current time: {}", e)))?;

    Ok(Value::Number(now.as_secs_f64()))
}

/// Returns the Unix timestamp in seconds for a given time string
fn time_unix_timestamp<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let time_str = string_arg!(&args, 0, "unix_timestamp")?;

    // Parse the datetime string (assumes ISO 8601 format)
    let dt = DateTime::parse_from_rfc3339(time_str.to_str().unwrap())
        .map_err(|e| VmError::RuntimeError(format!("Failed to parse timestamp: {}", e)))?;

    Ok(Value::Number(dt.timestamp() as f64))
}

/// Sleeps for the specified number of seconds
fn time_sleep<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let seconds = float_arg!(&args, 0, "sleep")?;

    if seconds < 0.0 {
        return Err(VmError::RuntimeError(
            "sleep: duration cannot be negative".into(),
        ));
    }

    let duration = Duration::from_secs_f64(seconds);
    thread::sleep(duration);

    Ok(Value::Nil)
}

/// Converts a timestamp to UTC datetime string
fn time_to_utc<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let timestamp = float_arg!(&args, 0, "to_utc")?;

    let dt = Utc
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .ok_or_else(|| VmError::RuntimeError("Invalid timestamp".into()))?;

    let formatted = dt.to_rfc3339();
    Ok(Value::IoString(Gc::new(state, formatted)))
}

/// Converts a timestamp to local datetime string
fn time_to_local<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let timestamp = float_arg!(&args, 0, "to_local")?;

    let dt = Local
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .ok_or_else(|| VmError::RuntimeError("Invalid timestamp".into()))?;

    let formatted = dt.to_rfc3339();
    Ok(Value::IoString(Gc::new(state, formatted)))
}

/// Formats a timestamp using the specified format string
fn time_format_datetime<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let timestamp = float_arg!(&args, 0, "format_datetime")?;
    let format = string_arg!(&args, 1, "format_datetime")?;

    let dt = Local
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .ok_or_else(|| VmError::RuntimeError("Invalid timestamp".into()))?;

    let formatted = dt.format(format.to_str().unwrap()).to_string();
    Ok(Value::IoString(Gc::new(state, formatted)))
}

/// Parses a datetime string using the specified format
fn time_parse_datetime<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let datetime_str = string_arg!(&args, 0, "parse_datetime")?;
    let format = string_arg!(&args, 1, "parse_datetime")?;

    let dt =
        NaiveDateTime::parse_from_str(datetime_str.to_str().unwrap(), format.to_str().unwrap())
            .map_err(|e| VmError::RuntimeError(format!("Failed to parse datetime: {}", e)))?;

    Ok(Value::Number(dt.and_utc().timestamp() as f64))
}

// Duration conversion functions
fn time_seconds<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let seconds = float_arg!(&args, 0, "seconds")?;
    Ok(Value::Number(seconds))
}

fn time_minutes<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let minutes = float_arg!(&args, 0, "minutes")?;
    Ok(Value::Number(minutes * 60.0))
}

fn time_hours<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let hours = float_arg!(&args, 0, "hours")?;
    Ok(Value::Number(hours * 3600.0))
}

fn time_days<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let days = float_arg!(&args, 0, "days")?;
    Ok(Value::Number(days * 86400.0))
}
