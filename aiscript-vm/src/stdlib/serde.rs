use gc_arena::Gc;
use std::fs;

use crate::{
    module::ModuleKind,
    string_arg,
    vm::{Context, State},
    NativeFn, Value, VmError,
};

pub fn create_serde_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.serde");

    let exports = [
        ("from_str", Value::NativeFunction(NativeFn(serde_from_str))),
        ("to_str", Value::NativeFunction(NativeFn(serde_to_str))),
        (
            "from_file",
            Value::NativeFunction(NativeFn(serde_from_file)),
        ),
        ("to_file", Value::NativeFunction(NativeFn(serde_to_file))),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect();

    ModuleKind::Native { name, exports }
}

fn serde_from_str<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.is_empty() || args.len() > 1 {
        return Err(VmError::RuntimeError(
            "from_str() takes exactly 1 argument".into(),
        ));
    }

    let json_str = string_arg!(&args, 0, "from_str")?;

    // Parse JSON string into serde_json::Value
    let parsed = serde_json::from_str(json_str.to_str().unwrap())
        .map_err(|e| VmError::RuntimeError(format!("Failed to parse JSON: {}", e)))?;

    // Convert serde_json::Value to AIScript Value
    Ok(Value::from_serde_value(state.get_context(), &parsed))
}

fn serde_to_str<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let (positional, keyword) = extract_keyword_args(&args)?;

    if positional.len() != 1 {
        return Err(VmError::RuntimeError(
            "to_str() requires value argument".into(),
        ));
    }

    let pretty = if let Some(pretty_val) = keyword.get("pretty") {
        match pretty_val {
            Value::Boolean(b) => *b,
            _ => {
                return Err(VmError::RuntimeError(
                    "pretty argument must be a boolean".into(),
                ))
            }
        }
    } else {
        false
    };

    // Convert AIScript Value to JSON value
    let json_value = to_json_value(&positional[0])?;

    // Convert to string with appropriate formatting
    let result = if pretty {
        serde_json::to_string_pretty(&json_value)
    } else {
        serde_json::to_string(&json_value)
    }
    .map_err(|e| VmError::RuntimeError(format!("Failed to serialize to JSON: {}", e)))?;

    Ok(Value::IoString(Gc::new(state, result)))
}

fn serde_from_file<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "from_file() takes exactly 1 argument".into(),
        ));
    }

    let path = string_arg!(&args, 0, "from_file")?;

    // Read file content
    let content = fs::read_to_string(path.to_str().unwrap())
        .map_err(|e| VmError::RuntimeError(format!("Failed to read file: {}", e)))?;

    // Parse JSON content
    let parsed = serde_json::from_str(&content)
        .map_err(|e| VmError::RuntimeError(format!("Failed to parse JSON from file: {}", e)))?;

    // Convert to AIScript Value
    Ok(Value::from_serde_value(state.get_context(), &parsed))
}

fn serde_to_file<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    // First extract keyword args
    let (positional, keyword) = extract_keyword_args(&args)?;

    if positional.len() != 2 {
        return Err(VmError::RuntimeError(
            "to_file() requires path and value arguments".into(),
        ));
    }

    let path = string_arg!(&positional, 0, "to_file")?;

    let pretty = if let Some(pretty_val) = keyword.get("pretty") {
        match pretty_val {
            Value::Boolean(b) => *b,
            _ => {
                return Err(VmError::RuntimeError(
                    "pretty argument must be a boolean".into(),
                ))
            }
        }
    } else {
        false
    };

    // Convert AIScript Value to JSON value
    let json_value = to_json_value(&positional[1])?;

    // Serialize to string with appropriate formatting
    let json_str = if pretty {
        serde_json::to_string_pretty(&json_value)
    } else {
        serde_json::to_string(&json_value)
    }
    .map_err(|e| VmError::RuntimeError(format!("Failed to serialize to JSON: {}", e)))?;

    // Write to file
    fs::write(path.to_str().unwrap(), json_str)
        .map_err(|e| VmError::RuntimeError(format!("Failed to write to file: {}", e)))?;

    Ok(Value::Boolean(true))
}

// Helper function to convert AIScript Value to serde_json::Value
fn to_json_value(value: &Value) -> Result<serde_json::Value, VmError> {
    match value {
        Value::Object(obj) => {
            let mut map = serde_json::Map::new();
            for (k, v) in &obj.borrow().fields {
                map.insert(k.to_string(), to_json_value(v)?);
            }
            Ok(serde_json::Value::Object(map))
        }
        Value::Array(arr) => {
            let values: Result<Vec<_>, _> = arr.borrow().iter().map(|v| to_json_value(v)).collect();
            Ok(serde_json::Value::Array(values?))
        }
        Value::Number(n) => Ok(serde_json::Value::Number(
            serde_json::Number::from_f64(*n)
                .ok_or_else(|| VmError::RuntimeError("Invalid number value for JSON".into()))?,
        )),
        Value::String(s) => Ok(serde_json::Value::String(s.to_string())),
        Value::IoString(s) => Ok(serde_json::Value::String(s.to_string())),
        Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Nil => Ok(serde_json::Value::Null),
        _ => Err(VmError::RuntimeError(
            "Value cannot be serialized to JSON".into(),
        )),
    }
}

// Helper function to extract keyword arguments from args vector
fn extract_keyword_args<'gc>(
    args: &[Value<'gc>],
) -> Result<
    (
        Vec<Value<'gc>>,
        std::collections::HashMap<String, Value<'gc>>,
    ),
    VmError,
> {
    let mut positional = Vec::new();
    let mut keyword = std::collections::HashMap::new();
    let mut i = 0;

    while i < args.len() {
        match (&args[i], args.get(i + 1)) {
            (Value::String(key), Some(value)) if i < args.len() - 1 => {
                // Check if this is a key-value pair for a named argument
                if key.to_str().unwrap() == "pretty" {
                    keyword.insert("pretty".to_string(), *value);
                    i += 2;
                    continue;
                }
            }
            _ => {}
        }
        positional.push(args[i]);
        i += 1;
    }

    Ok((positional, keyword))
}
