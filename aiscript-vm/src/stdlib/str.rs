use gc_arena::{Gc, Mutation};
use regex::Regex;

use crate::{float_arg, module::ModuleKind, string_arg, vm::Context, Value, VmError};

pub fn create_str_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.str");

    let exports = [
        // Length and checks
        (ctx.intern(b"len"), Value::NativeFunction(str_len)),
        (ctx.intern(b"is_empty"), Value::NativeFunction(str_is_empty)),
        // Case conversion
        (
            ctx.intern(b"to_uppercase"),
            Value::NativeFunction(str_to_uppercase),
        ),
        (
            ctx.intern(b"to_lowercase"),
            Value::NativeFunction(str_to_lowercase),
        ),
        // Trim functions
        (ctx.intern(b"trim"), Value::NativeFunction(str_trim)),
        (
            ctx.intern(b"trim_start"),
            Value::NativeFunction(str_trim_start),
        ),
        (ctx.intern(b"trim_end"), Value::NativeFunction(str_trim_end)),
        // Contains and position
        (ctx.intern(b"contains"), Value::NativeFunction(str_contains)),
        (
            ctx.intern(b"starts_with"),
            Value::NativeFunction(str_starts_with),
        ),
        (
            ctx.intern(b"ends_with"),
            Value::NativeFunction(str_ends_with),
        ),
        (ctx.intern(b"index_of"), Value::NativeFunction(str_index_of)),
        (
            ctx.intern(b"last_index_of"),
            Value::NativeFunction(str_last_index_of),
        ),
        // Substring and slicing
        (
            ctx.intern(b"substring"),
            Value::NativeFunction(str_substring),
        ),
        (ctx.intern(b"slice"), Value::NativeFunction(str_slice)),
        // Split and join
        // (ctx.intern(b"split"), Value::NativeFunction(str_split)),
        (ctx.intern(b"join"), Value::NativeFunction(str_join)),
        // Regex operations
        (
            ctx.intern(b"regex_match"),
            Value::NativeFunction(str_regex_match),
        ),
        (
            ctx.intern(b"regex_replace"),
            Value::NativeFunction(str_regex_replace),
        ),
        // Misc string operations
        (ctx.intern(b"repeat"), Value::NativeFunction(str_repeat)),
        (ctx.intern(b"reverse"), Value::NativeFunction(str_reverse)),
        (ctx.intern(b"replace"), Value::NativeFunction(str_replace)),
    ]
    .into_iter()
    .collect();

    ModuleKind::Native { name, exports }
}

// Length and checks
fn str_len<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "len")?;
    Ok(Value::Number(s.len() as f64))
}

fn str_is_empty<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "is_empty")?;
    Ok(Value::Boolean(s.is_empty()))
}

// Case conversion
fn str_to_uppercase<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "to_uppercase")?;
    let upper = s.to_str().unwrap().to_uppercase();
    // Ok(Value::String(_mc.intern(upper.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, upper)))
}

fn str_to_lowercase<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "to_lowercase")?;
    let lower = s.to_str().unwrap().to_lowercase();
    // Ok(Value::String(_mc.intern(lower.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, lower)))
}

// Trim functions
fn str_trim<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "trim")?;
    let trimmed = s.to_str().unwrap().trim();
    // Ok(Value::String(_mc.intern(trimmed.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, trimmed.to_owned())))
}

fn str_trim_start<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "trim_start")?;
    let trimmed = s.to_str().unwrap().trim_start();
    // Ok(Value::String(_mc.intern(trimmed.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, trimmed.to_owned())))
}

fn str_trim_end<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "trim_end")?;
    let trimmed = s.to_str().unwrap().trim_end();
    // Ok(Value::String(_mc.intern(trimmed.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, trimmed.to_owned())))
}

// Contains and position functions
fn str_contains<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "contains")?;
    let substr = string_arg!(&args, 1, "contains")?;
    Ok(Value::Boolean(
        s.to_str().unwrap().contains(substr.to_str().unwrap()),
    ))
}

fn str_starts_with<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "starts_with")?;
    let prefix = string_arg!(&args, 1, "starts_with")?;
    Ok(Value::Boolean(
        s.to_str().unwrap().starts_with(prefix.to_str().unwrap()),
    ))
}

fn str_ends_with<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "ends_with")?;
    let suffix = string_arg!(&args, 1, "ends_with")?;
    Ok(Value::Boolean(
        s.to_str().unwrap().ends_with(suffix.to_str().unwrap()),
    ))
}

// Find and position functions
fn str_index_of<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "index_of")?;
    let substr = string_arg!(&args, 1, "index_of")?;

    // Optional start position
    let start = if args.len() > 2 {
        float_arg!(&args, 2, "index_of")? as usize
    } else {
        0
    };

    let s = s.to_str().unwrap();
    let substr = substr.to_str().unwrap();

    if start > s.len() {
        return Ok(Value::Number(-1.0));
    }

    match s[start..].find(substr) {
        Some(pos) => Ok(Value::Number((pos + start) as f64)),
        None => Ok(Value::Number(-1.0)),
    }
}

fn str_last_index_of<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "last_index_of")?;
    let substr = string_arg!(&args, 1, "last_index_of")?;

    let s = s.to_str().unwrap();
    let substr = substr.to_str().unwrap();

    match s.rfind(substr) {
        Some(pos) => Ok(Value::Number(pos as f64)),
        None => Ok(Value::Number(-1.0)),
    }
}

// Substring and slicing
fn str_substring<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "substring")?;
    let start = float_arg!(&args, 1, "substring")? as usize;

    let end = if args.len() > 2 {
        float_arg!(&args, 2, "substring")? as usize
    } else {
        s.len() as usize
    };

    let s = s.to_str().unwrap();

    // Handle start and end bounds
    let start = start.min(s.len());
    let end = end.min(s.len());
    let start = start.min(end); // Ensure start <= end

    // Ok(Value::String(_mc.intern(s[start..end].as_bytes())))
    Ok(Value::IoString(Gc::new(mc, s[start..end].to_owned())))
}

fn str_slice<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "slice")?;
    let start = float_arg!(&args, 1, "slice")? as isize;

    let end = if args.len() > 2 {
        float_arg!(&args, 2, "slice")? as isize
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
    Ok(Value::IoString(Gc::new(mc, s[start..end].to_owned())))
}

// Split and join
// fn str_split<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
//     let s = string_arg!(&args, 0, "split")?;
//     let delimiter = string_arg!(&args, 1, "split")?;

//     let parts: Vec<&str> = s
//         .to_str()
//         .unwrap()
//         .split(delimiter.to_str().unwrap())
//         // .map(|part| Value::IoString(_mc.intern(part.as_bytes())))
//         .collect();

//     // Convert to array once array type is implemented
//     Ok(Value::String(_mc.intern(format!("{:?}", parts).as_bytes())))
// }

fn str_join<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let separator = string_arg!(&args, 0, "join")?;

    if args.len() < 2 {
        return Err(VmError::RuntimeError(
            "join: expected at least two arguments".into(),
        ));
    }

    let mut result = String::new();
    for (i, arg) in args.iter().skip(1).enumerate() {
        if i > 0 {
            result.push_str(separator.to_str().unwrap());
        }
        match arg {
            Value::String(s) => result.push_str(s.to_str().unwrap()),
            _ => {
                return Err(VmError::RuntimeError(format!(
                    "join: argument {} must be a string",
                    i + 2
                )))
            }
        }
    }

    Ok(Value::IoString(Gc::new(mc, result)))
}

// Regex operations
fn str_regex_match<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "regex_match")?;
    let pattern = string_arg!(&args, 1, "regex_match")?;

    let regex = Regex::new(pattern.to_str().unwrap())
        .map_err(|e| VmError::RuntimeError(format!("Invalid regex pattern: {}", e)))?;

    Ok(Value::Boolean(regex.is_match(s.to_str().unwrap())))
}

fn str_regex_replace<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "regex_replace")?;
    let pattern = string_arg!(&args, 1, "regex_replace")?;
    let replacement = string_arg!(&args, 2, "regex_replace")?;

    let regex = Regex::new(pattern.to_str().unwrap())
        .map_err(|e| VmError::RuntimeError(format!("Invalid regex pattern: {}", e)))?;

    let result = regex
        .replace_all(s.to_str().unwrap(), replacement.to_str().unwrap())
        .into_owned();

    // Ok(Value::String(_mc.intern(result.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, result)))
}

// Misc string operations
fn str_repeat<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "repeat")?;
    let count = float_arg!(&args, 1, "repeat")? as usize;

    let repeated = s.to_str().unwrap().repeat(count);
    // Ok(Value::String(_mc.intern(repeated.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, repeated)))
}

fn str_reverse<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "reverse")?;
    let reversed: String = s.to_str().unwrap().chars().rev().collect();
    // Ok(Value::String(_mc.intern(reversed.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, reversed)))
}

fn str_replace<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let s = string_arg!(&args, 0, "replace")?;
    let from = string_arg!(&args, 1, "replace")?;
    let to = string_arg!(&args, 2, "replace")?;

    let result = s
        .to_str()
        .unwrap()
        .replace(from.to_str().unwrap(), to.to_str().unwrap());
    // Ok(Value::String(_mc.intern(result.as_bytes())))
    Ok(Value::IoString(Gc::new(mc, result)))
}
