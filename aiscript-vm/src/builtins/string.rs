use std::collections::HashMap;

use gc_arena::{Gc, Mutation};
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
        // ("split", BuiltinMethod(split)),
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
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let upper = receiver.as_string()?.to_str().unwrap().to_uppercase();
    // Ok(Value::String(_mc.intern(upper.as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, upper)))
}

fn to_lowercase<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let lower = receiver.as_string()?.to_str().unwrap().to_lowercase();
    // Ok(Value::String(_mc.intern(lower.as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, lower)))
}

// Trim functions
fn trim<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let trimmed = receiver.as_string()?.to_str().unwrap().trim();
    // Ok(Value::String(_mc.intern(trimmed.as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, trimmed.to_owned())))
}

fn trim_start<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let trimmed = receiver.as_string()?.to_str().unwrap().trim_start();
    // Ok(Value::String(_mc.intern(trimmed.as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, trimmed.to_owned())))
}

fn trim_end<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let trimmed = receiver.as_string()?.to_str().unwrap().trim_end();
    // Ok(Value::String(_mc.intern(trimmed.as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, trimmed.to_owned())))
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

    let s = receiver.as_string()?.to_str().unwrap();
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

    let s = receiver.as_string()?.to_str().unwrap();
    let substr = substr.to_str().unwrap();

    match s.rfind(substr) {
        Some(pos) => Ok(Value::Number(pos as f64)),
        None => Ok(Value::Number(-1.0)),
    }
}

// Substring and slicing
fn substring<'gc>(
    _mc: &'gc Mutation<'gc>,
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

    // Ok(Value::String(_mc.intern(s[start..end].as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, s[start..end].to_owned())))
}

fn slice<'gc>(
    _mc: &'gc Mutation<'gc>,
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

    // Ok(Value::String(_mc.intern(s[start..end].as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, s[start..end].to_owned())))
}

// Split and join
// fn split<'gc>(_mc: &'gc Mutation<'gc>, receiver: Value<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
//     let delimiter = string_arg!(&args, 0, "split")?;

//     let parts: Vec<&str> = receiver.as_string()?
//         .to_str()
//         .unwrap()
//         .split(delimiter.to_str().unwrap())
//         // .map(|part| Value::IoString(_mc.intern(part.as_bytes())))
//         .collect();

//     // Convert to array once array type is implemented
//     Ok(Value::String(_mc.intern(format!("{:?}", parts).as_bytes())))
// }

fn join<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let separator = receiver.as_string()?;

    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "join: expected at least one argument".into(),
        ));
    }

    let mut result = String::new();
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            result.push_str(separator.to_str().unwrap());
        }
        match arg {
            Value::String(s) => result.push_str(s.to_str().unwrap()),
            _ => {
                return Err(VmError::RuntimeError(format!(
                    "join: argument {} must be a string",
                    i + 1
                )))
            }
        }
    }

    Ok(Value::IoString(Gc::new(_mc, result)))
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
        regex.is_match(receiver.as_string()?.to_str().unwrap()),
    ))
}

fn regex_replace<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let pattern = string_arg!(&args, 0, "regex_replace")?;
    let replacement = string_arg!(&args, 1, "regex_replace")?;

    let regex = Regex::new(pattern.to_str().unwrap())
        .map_err(|e| VmError::RuntimeError(format!("Invalid regex pattern: {}", e)))?;

    let result = regex
        .replace_all(
            receiver.as_string()?.to_str().unwrap(),
            replacement.to_str().unwrap(),
        )
        .into_owned();

    // Ok(Value::String(_mc.intern(result.as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, result)))
}

// Misc string operations
fn repeat<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let count = float_arg!(&args, 0, "repeat")? as usize;

    let repeated = receiver.as_string()?.to_str().unwrap().repeat(count);
    // Ok(Value::String(_mc.intern(repeated.as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, repeated)))
}

fn reverse<'gc>(
    _mc: &'gc Mutation<'gc>,
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
    // Ok(Value::String(_mc.intern(reversed.as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, reversed)))
}

fn replace<'gc>(
    _mc: &'gc Mutation<'gc>,
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
    // Ok(Value::String(_mc.intern(result.as_bytes())))
    Ok(Value::IoString(Gc::new(_mc, result)))
}
