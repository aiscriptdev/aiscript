use crate::{vm::State, Value, VmError};
use gc_arena::Gc;
use std::io::{self, Write};

mod convert;
mod format;
mod function;
mod print;

use convert::*;
use format::format;
use function::*;
use print::print;

pub(crate) fn define_native_functions(state: &mut State) {
    state.define_native_function("abs", abs);
    state.define_native_function("all", all);
    state.define_native_function("any", any);
    state.define_native_function("ascii", ascii);
    state.define_native_function("bin", bin);
    state.define_native_function("bool", bool);
    state.define_native_function("callable", callable);
    state.define_native_function("chr", chr);
    state.define_native_function("filter", filter);
    state.define_native_function("float", float);
    state.define_native_function("format", format);
    state.define_native_function("hex", hex);
    state.define_native_function("input", input);
    state.define_native_function("int", int);
    state.define_native_function("len", len);
    state.define_native_function("map", map);
    state.define_native_function("max", max);
    state.define_native_function("min", min);
    state.define_native_function("oct", oct);
    state.define_native_function("ord", ord);
    state.define_native_function("print", print);
    state.define_native_function("round", round);
    state.define_native_function("str", str);
    state.define_native_function("sum", sum);
    state.define_native_function("zip", zip);
}

fn abs<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn len<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn any<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn all<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn min<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn max<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn round<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn sum<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn input<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

    Ok(Value::IoString(Gc::new(state, input)))
}

fn callable<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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
