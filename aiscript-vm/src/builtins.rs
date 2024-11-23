use gc_arena::{Gc, Mutation};
use std::io::{self, Write};

use crate::{Value, VmError};

pub(crate) fn abs<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "abs() takes exactly one argument".into(),
        ));
    }

    match args[0] {
        Value::Number(n) => Ok(n.abs().into()),
        _ => Err(VmError::RuntimeError(
            "abs() argument must be a number".into(),
        )),
    }
}

pub(crate) fn len<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "len() takes exactly one argument".into(),
        ));
    }

    match &args[0] {
        Value::String(s) => Ok(Value::Number(s.len() as f64)),
        Value::IoString(s) => Ok(Value::Number(s.len() as f64)),
        Value::Array(arr) => Ok(Value::Number(arr.borrow().len() as f64)),
        Value::Object(obj) => Ok(Value::Number(obj.borrow().fields.len() as f64)),
        _ => Err(VmError::RuntimeError(
            "len() argument must be a string, array or object".into(),
        )),
    }
}

pub(crate) fn any<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "any() takes exactly one argument".into(),
        ));
    }

    match &args[0] {
        Value::Array(arr) => {
            let arr = arr.borrow();
            Ok(Value::Boolean(arr.iter().any(|x| x.is_true())))
        }
        _ => Err(VmError::RuntimeError(
            "any() argument must be an array".into(),
        )),
    }
}

pub(crate) fn all<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "all() takes exactly one argument".into(),
        ));
    }

    match &args[0] {
        Value::Array(arr) => {
            let arr = arr.borrow();
            Ok(Value::Boolean(arr.iter().all(|x| x.is_true())))
        }
        _ => Err(VmError::RuntimeError(
            "all() argument must be an array".into(),
        )),
    }
}

pub(crate) fn min<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "min() takes at least one argument".into(),
        ));
    }

    if args.len() == 1 {
        // If single argument, it should be an array
        match &args[0] {
            Value::Array(arr) => {
                let arr = arr.borrow();
                if arr.is_empty() {
                    return Err(VmError::RuntimeError("min() arg is an empty array".into()));
                }
                arr.iter()
                    .min_by(|a, b| {
                        if let (Value::Number(x), Value::Number(y)) = (a, b) {
                            x.partial_cmp(y).unwrap()
                        } else {
                            panic!("min() array elements must be numbers")
                        }
                    })
                    .copied()
                    .ok_or_else(|| VmError::RuntimeError("min() array must not be empty".into()))
            }
            _ => Err(VmError::RuntimeError(
                "single argument to min() must be an array".into(),
            )),
        }
    } else {
        // Multiple arguments case
        args.iter()
            .min_by(|a, b| {
                if let (Value::Number(x), Value::Number(y)) = (a, b) {
                    x.partial_cmp(y).unwrap()
                } else {
                    panic!("min() arguments must be numbers")
                }
            })
            .copied()
            .ok_or_else(|| VmError::RuntimeError("min() arguments must be numbers".into()))
    }
}

pub(crate) fn max<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "max() takes at least one argument".into(),
        ));
    }

    if args.len() == 1 {
        // If single argument, it should be an array
        match &args[0] {
            Value::Array(arr) => {
                let arr = arr.borrow();
                if arr.is_empty() {
                    return Err(VmError::RuntimeError("max() arg is an empty array".into()));
                }
                arr.iter()
                    .max_by(|a, b| {
                        if let (Value::Number(x), Value::Number(y)) = (a, b) {
                            x.partial_cmp(y).unwrap()
                        } else {
                            panic!("max() array elements must be numbers")
                        }
                    })
                    .copied()
                    .ok_or_else(|| VmError::RuntimeError("max() array must not be empty".into()))
            }
            _ => Err(VmError::RuntimeError(
                "single argument to max() must be an array".into(),
            )),
        }
    } else {
        // Multiple arguments case
        args.iter()
            .max_by(|a, b| {
                if let (Value::Number(x), Value::Number(y)) = (a, b) {
                    x.partial_cmp(y).unwrap()
                } else {
                    panic!("max() arguments must be numbers")
                }
            })
            .copied()
            .ok_or_else(|| VmError::RuntimeError("max() arguments must be numbers".into()))
    }
}

pub(crate) fn round<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "round() takes exactly one argument".into(),
        ));
    }

    match args[0] {
        Value::Number(n) => Ok(n.round().into()),
        _ => Err(VmError::RuntimeError(
            "round() argument must be a number".into(),
        )),
    }
}

pub(crate) fn sum<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "sum() takes exactly one argument".into(),
        ));
    }

    match &args[0] {
        Value::Array(arr) => {
            let arr = arr.borrow();
            let mut sum = 0.0;
            for value in arr.iter() {
                if let Value::Number(n) = value {
                    sum += n;
                } else {
                    return Err(VmError::RuntimeError(
                        "sum() array elements must be numbers".into(),
                    ));
                }
            }
            Ok(sum.into())
        }
        _ => Err(VmError::RuntimeError(
            "sum() argument must be an array".into(),
        )),
    }
}

pub(crate) fn input<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    // If a prompt is provided, print it without a newline
    if let Some(prompt) = args.first() {
        match prompt {
            Value::String(s) => print!("{}", s),
            Value::IoString(s) => print!("{}", s),
            _ => {
                return Err(VmError::RuntimeError(
                    "input() prompt must be a string".into(),
                ))
            }
        }
        io::stdout().flush().unwrap();
    }

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| VmError::RuntimeError(format!("Failed to read input: {}", e)))?;

    // Trim the trailing newline
    input = input.trim_end().to_string();

    Ok(Value::IoString(Gc::new(mc, input)))
}

pub(crate) fn print<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        println!();
        return Ok(Value::Nil);
    }

    // Print each argument with a space between them
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            print!(" ");
        }
        print!("{}", arg);
    }
    // println!();

    Ok(Value::Nil)
}

pub(crate) fn bool<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "bool() takes exactly one argument".into(),
        ));
    }

    match &args[0] {
        Value::Number(n) => Ok(Value::Boolean(*n != 0.0)),
        Value::String(s) => Ok(Value::Boolean(!s.is_empty())),
        Value::IoString(s) => Ok(Value::Boolean(!s.is_empty())),
        Value::Boolean(b) => Ok(Value::Boolean(*b)),
        Value::Nil => Ok(Value::Boolean(false)),
        Value::Array(arr) => Ok(Value::Boolean(!arr.borrow().is_empty())),
        Value::Object(obj) => Ok(Value::Boolean(!obj.borrow().fields.is_empty())),
        Value::Class(_)
        | Value::Instance(_)
        | Value::Closure(_)
        | Value::BoundMethod(_)
        | Value::NativeFunction(_)
        | Value::Module(_)
        | Value::Agent(_) => Ok(Value::Boolean(true)),
    }
}

pub(crate) fn float<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "float() takes exactly one argument".into(),
        ));
    }

    match &args[0] {
        Value::Number(n) => Ok(Value::Number(*n)),
        Value::String(s) /*| Value::IoString(s)*/ => {
            let s = s.to_string();
            match s.parse::<f64>() {
                Ok(n) => Ok(Value::Number(n)),
                Err(_) => Err(VmError::RuntimeError(format!(
                    "could not convert string to float: '{}'",
                    s
                ))),
            }
        }
        Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
        Value::Nil => Ok(Value::Number(0.0)),
        _ => Err(VmError::RuntimeError(format!(
            "could not convert {} to float",
            args[0]
        ))),
    }
}

pub(crate) fn int<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "int() takes exactly one argument".into(),
        ));
    }

    match &args[0] {
        Value::Number(n) => Ok(Value::Number(n.trunc())),
        Value::String(s) /*| Value::IoString(s)*/ => {
            let s = s.to_string();
            match s.parse::<f64>() {
                Ok(n) => Ok(Value::Number(n.trunc())),
                Err(_) => Err(VmError::RuntimeError(format!(
                    "could not convert string to int: '{}'",
                    s
                ))),
            }
        }
        Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
        Value::Nil => Ok(Value::Number(0.0)),
        _ => Err(VmError::RuntimeError(format!(
            "could not convert {} to int",
            args[0]
        ))),
    }
}

pub(crate) fn ascii<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "ascii() takes exactly one argument".into(),
        ));
    }

    let s = match &args[0] {
        Value::String(s) /*| Value::IoString(s)*/ => s.to_string(),
        _ => args[0].to_string(),
    };

    // Create ASCII representation
    let mut result = String::new();
    for c in s.chars() {
        if c.is_ascii() {
            result.push(c);
        } else {
            // Format non-ASCII characters like Python does
            result.push_str(&format!("\\x{:02x}", c as u32));
        }
    }
    Ok(Value::IoString(Gc::new(mc, result)))
}

pub(crate) fn chr<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "chr() takes exactly one argument".into(),
        ));
    }

    let num = match args[0] {
        Value::Number(n) => n as u32,
        _ => {
            return Err(VmError::RuntimeError(
                "chr() argument must be an integer".into(),
            ))
        }
    };

    if num > 0x10FFFF {
        return Err(VmError::RuntimeError(
            "chr() arg not in range(0x110000)".to_string(),
        ));
    }

    match char::from_u32(num) {
        Some(c) => Ok(Value::IoString(Gc::new(mc, c.to_string()))),
        None => Err(VmError::RuntimeError(
            "chr() arg not in range(0x110000)".to_string(),
        )),
    }
}

pub(crate) fn ord<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "ord() takes exactly one argument".into(),
        ));
    }

    let s = match &args[0] {
        Value::String(s) /*| Value::IoString(s)*/ => s.to_string(),
        _ => {
            return Err(VmError::RuntimeError(
                "ord() argument must be a string".into(),
            ))
        }
    };

    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(Value::Number(c as u32 as f64)),
        (None, _) => Err(VmError::RuntimeError(
            "ord() argument must be a string of length 1".into(),
        )),
        (Some(_), Some(_)) => Err(VmError::RuntimeError(
            "ord() argument must be a string of length 1".into(),
        )),
    }
}

pub(crate) fn str<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "str() takes exactly one argument".into(),
        ));
    }

    let s = args[0].to_string();
    Ok(Value::IoString(Gc::new(mc, s)))
}

pub(crate) fn bin<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "bin() takes exactly one argument".into(),
        ));
    }

    let num = match args[0] {
        Value::Number(n) => n as i64,
        _ => {
            return Err(VmError::RuntimeError(
                "bin() argument must be an integer".into(),
            ))
        }
    };

    Ok(Value::IoString(Gc::new(mc, format!("0b{:b}", num))))
}

pub(crate) fn hex<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "hex() takes exactly one argument".into(),
        ));
    }

    let num = match args[0] {
        Value::Number(n) => n as i64,
        _ => {
            return Err(VmError::RuntimeError(
                "hex() argument must be an integer".into(),
            ))
        }
    };

    Ok(Value::IoString(Gc::new(mc, format!("0x{:x}", num))))
}

pub(crate) fn oct<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "oct() takes exactly one argument".into(),
        ));
    }

    let num = match args[0] {
        Value::Number(n) => n as i64,
        _ => {
            return Err(VmError::RuntimeError(
                "oct() argument must be an integer".into(),
            ))
        }
    };

    Ok(Value::IoString(Gc::new(mc, format!("0o{:o}", num))))
}

pub(crate) fn callable<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "callable() takes exactly one argument".into(),
        ));
    }

    Ok(Value::Boolean(matches!(
        args[0],
        Value::Closure(_) | Value::NativeFunction(_) | Value::BoundMethod(_) | Value::Class(_)
    )))
}
