use std::collections::HashMap;

use aiscript_arena::{Gc, Mutation};
use regex::Regex;

use crate::BuiltinMethod;
use crate::{float_arg, string_arg, vm::Context, Value, VmError};

use crate::string::InternedString;

pub(crate) fn define_string_methods(ctx: Context) -> HashMap<InternedString, BuiltinMethod> {
    [
        ("is_empty", BuiltinMethod(is_empty)),
        // Case conversion
        ("to_uppercase", BuiltinMethod(to_uppercase)),
        ("to_lowercase", BuiltinMethod(to_lowercase)),
        // Trim functions
        ("trim", BuiltinMethod(trim)),
        ("trim_start", BuiltinMethod(trim_start)),
        ("trim_end", BuiltinMethod(trim_end)),
        // Contains and position
        ("contains", BuiltinMethod(contains)),
        ("starts_with", BuiltinMethod(starts_with)),
        ("ends_with", BuiltinMethod(ends_with)),
        ("index_of", BuiltinMethod(index_of)),
        ("last_index_of", BuiltinMethod(last_index_of)),
        // Substring and slicing
        ("substring", BuiltinMethod(substring)),
        ("slice", BuiltinMethod(slice)),
        // Split and join
        ("split", BuiltinMethod(split)),
        ("join", BuiltinMethod(join)),
        // Regex operations
        ("regex_match", BuiltinMethod(regex_match)),
        ("regex_replace", BuiltinMethod(regex_replace)),
        // Misc string operations
        ("repeat", BuiltinMethod(repeat)),
        ("reverse", BuiltinMethod(reverse)),
        ("replace", BuiltinMethod(replace)),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect()
}

fn is_empty<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    Ok(Value::Boolean(receiver.as_string()?.is_empty()))
}

// Case conversion
fn to_uppercase<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let upper = receiver.as_string_value()?.as_str().to_uppercase();
    // Ok(Value::String(mc.intern(upper.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, upper)))
}

fn to_lowercase<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let lower = receiver.as_string_value()?.as_str().to_lowercase();
    // Ok(Value::String(mc.intern(lower.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, lower)))
}

// Trim functions
fn trim<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = receiver.as_string_value()?;
    let trimmed = s.as_str().trim();
    // Ok(Value::String(mc.intern(trimmed.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, trimmed.to_owned())))
}

fn trim_start<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = receiver.as_string_value()?;
    let trimmed = s.as_str().trim_start();
    // Ok(Value::String(mc.intern(trimmed.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, trimmed.to_owned())))
}

fn trim_end<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = receiver.as_string_value()?;
    let trimmed = s.as_str().trim_end();
    // Ok(Value::String(mc.intern(trimmed.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, trimmed.to_owned())))
}

// Contains and position functions
fn contains<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let substr = string_arg!(&args, 0, "contains")?;
    Ok(Value::Boolean(
        receiver
            .as_string()?
            .to_str()
            .unwrap()
            .contains(substr.to_str().unwrap()),
    ))
}

fn starts_with<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let prefix = string_arg!(&args, 0, "starts_with")?;
    Ok(Value::Boolean(
        receiver
            .as_string()?
            .to_str()
            .unwrap()
            .starts_with(prefix.to_str().unwrap()),
    ))
}

fn ends_with<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let suffix = string_arg!(&args, 0, "ends_with")?;
    Ok(Value::Boolean(
        receiver
            .as_string()?
            .to_str()
            .unwrap()
            .ends_with(suffix.to_str().unwrap()),
    ))
}

// Find and position functions
fn index_of<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let substr = string_arg!(&args, 0, "index_of")?;

    // Optional start position
    let start = if args.len() > 1 {
        float_arg!(&args, 1, "index_of")? as usize
    } else {
        0
    };

    let s = receiver.as_string_value()?;
    let s = s.as_str();
    let substr = substr.to_str().unwrap();

    if start > s.len() {
        return Ok(Value::Number(-1.0));
    }

    match s[start..].find(substr) {
        Some(pos) => Ok(Value::Number((pos + start) as f64)),
        None => Ok(Value::Number(-1.0)),
    }
}

fn last_index_of<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let substr = string_arg!(&args, 0, "last_index_of")?;

    let s = receiver.as_string_value()?;
    let s = s.as_str();
    let substr = substr.to_str().unwrap();

    match s.rfind(substr) {
        Some(pos) => Ok(Value::Number(pos as f64)),
        None => Ok(Value::Number(-1.0)),
    }
}

// Substring and slicing
fn substring<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let start = float_arg!(&args, 0, "substring")? as usize;

    let s = receiver.as_string()?;
    let end = if args.len() > 1 {
        float_arg!(&args, 1, "substring")? as usize
    } else {
        s.len() as usize
    };

    let s = s.to_str().unwrap();

    // Handle start and end bounds
    let start = start.min(s.len());
    let end = end.min(s.len());
    let start = start.min(end); // Ensure start <= end

    // Ok(Value::String(mc.intern(s[start..end].as_bytes())))
    Ok(Value::IoString(Gc::new(mc, s[start..end].to_owned())))
}

fn slice<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let start = float_arg!(&args, 0, "slice")? as isize;
    let s = receiver.as_string()?;
    let end = if args.len() > 1 {
        float_arg!(&args, 1, "slice")? as isize
    } else {
        s.len() as isize
    };

    let s = s.to_str().unwrap();
    let len = s.len() as isize;

    // Convert negative indices to positive
    let start = if start < 0 {
        (len + start).max(0)
    } else {
        start.min(len)
    } as usize;
    let end = if end < 0 {
        (len + end).max(0)
    } else {
        end.min(len)
    } as usize;
    let start = start.min(end); // Ensure start <= end

    // Ok(Value::String(mc.intern(s[start..end].as_bytes())))
    Ok(Value::IoString(Gc::new(mc, s[start..end].to_owned())))
}

// Split and join
fn split<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let delimiter = string_arg!(&args, 0, "split")?.to_str().unwrap();

    let s = receiver.as_string_value()?;
    let parts = s
        .as_str()
        .split(delimiter)
        .map(|part| Value::IoString(Gc::new(mc, part.to_string())))
        .collect();

    // Convert to array
    Ok(Value::array(mc, parts))
}

fn join<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let receiver = receiver.as_string_value()?;
    let separator = receiver.as_str();

    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "join: expected exactly one array argument".into(),
        ));
    }

    // Get the array from args[0]
    let vec = match &args[0] {
        Value::List(list) => &list.borrow().data,
        _ => {
            return Err(VmError::RuntimeError(
                "join: argument must be an array".into(),
            ))
        }
    };

    let mut result = String::new();
    for (i, value) in vec.iter().enumerate() {
        if i > 0 {
            result.push_str(separator);
        }
        match value {
            Value::String(s) => result.push_str(s.to_str().unwrap()),
            Value::IoString(s) => result.push_str(s),
            _ => {
                return Err(VmError::RuntimeError(format!(
                    "join: array element {} must be a string",
                    i
                )))
            }
        }
    }

    Ok(Value::IoString(Gc::new(mc, result)))
}

// Regex operations
fn regex_match<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let pattern = string_arg!(&args, 0, "regex_match")?;

    let regex = Regex::new(pattern.to_str().unwrap())
        .map_err(|e| VmError::RuntimeError(format!("Invalid regex pattern: {}", e)))?;

    Ok(Value::Boolean(
        regex.is_match(receiver.as_string_value()?.as_str()),
    ))
}

fn regex_replace<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let pattern = string_arg!(&args, 0, "regex_replace")?;
    let replacement = string_arg!(&args, 1, "regex_replace")?;

    let regex = Regex::new(pattern.to_str().unwrap())
        .map_err(|e| VmError::RuntimeError(format!("Invalid regex pattern: {}", e)))?;

    let result = regex
        .replace_all(
            receiver.as_string_value()?.as_str(),
            replacement.to_str().unwrap(),
        )
        .into_owned();

    // Ok(Value::String(mc.intern(result.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, result)))
}

// Misc string operations
fn repeat<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let count = float_arg!(&args, 0, "repeat")? as usize;

    let repeated = receiver.as_string_value()?.as_str().repeat(count);
    // Ok(Value::String(mc.intern(repeated.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, repeated)))
}

fn reverse<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let reversed: String = receiver
        .as_string()?
        .to_str()
        .unwrap()
        .chars()
        .rev()
        .collect();
    // Ok(Value::String(mc.intern(reversed.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, reversed)))
}

fn replace<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let from = string_arg!(&args, 0, "replace")?;
    let to = string_arg!(&args, 1, "replace")?;

    let result = receiver
        .as_string()?
        .to_str()
        .unwrap()
        .replace(from.to_str().unwrap(), to.to_str().unwrap());
    // Ok(Value::String(mc.intern(result.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, result)))
}
