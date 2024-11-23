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
