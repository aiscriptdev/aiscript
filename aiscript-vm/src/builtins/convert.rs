use crate::{vm::State, Value, VmError};
use gc_arena::Gc;

pub(super) fn bool<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "bool() takes exactly one argument.".into(),
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

pub(super) fn float<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "float() takes exactly one argument.".into(),
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

pub(super) fn int<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "int() takes exactly one argument.".into(),
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

pub(super) fn ascii<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "ascii() takes exactly one argument.".into(),
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
    Ok(Value::IoString(Gc::new(state, result)))
}

pub(super) fn chr<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "chr() takes exactly one argument.".into(),
        ));
    }

    let num = match args[0] {
        Value::Number(n) => n as u32,
        _ => {
            return Err(VmError::RuntimeError(
                "chr() argument must be an integer.".into(),
            ))
        }
    };

    if num > 0x10FFFF {
        return Err(VmError::RuntimeError(
            "chr() arg not in range(0x110000)".to_string(),
        ));
    }

    match char::from_u32(num) {
        Some(c) => Ok(Value::IoString(Gc::new(state, c.to_string()))),
        None => Err(VmError::RuntimeError(
            "chr() arg not in range(0x110000)".to_string(),
        )),
    }
}

pub(super) fn ord<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "ord() takes exactly one argument.".into(),
        ));
    }

    let s = match &args[0] {
        Value::String(s) /*| Value::IoString(s)*/ => s.to_string(),
        _ => {
            return Err(VmError::RuntimeError(
                "ord() argument must be a string.".into(),
            ))
        }
    };

    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(Value::Number(c as u32 as f64)),
        (None, _) => Err(VmError::RuntimeError(
            "ord() argument must be a string of length 1.".into(),
        )),
        (Some(_), Some(_)) => Err(VmError::RuntimeError(
            "ord() argument must be a string of length 1.".into(),
        )),
    }
}

pub(super) fn str<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "str() takes exactly one argument.".into(),
        ));
    }

    let s = args[0].to_string();
    Ok(Value::IoString(Gc::new(state, s)))
}

pub(super) fn bin<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "bin() takes exactly one argument.".into(),
        ));
    }

    let num = match args[0] {
        Value::Number(n) => n as i64,
        _ => {
            return Err(VmError::RuntimeError(
                "bin() argument must be an integer.".into(),
            ))
        }
    };

    // Handle negative numbers like Python does
    if num < 0 {
        Ok(Value::IoString(Gc::new(
            state,
            format!("-0b{:b}", num.abs()),
        )))
    } else {
        Ok(Value::IoString(Gc::new(state, format!("0b{:b}", num))))
    }
}

pub(super) fn hex<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "hex() takes exactly one argument.".into(),
        ));
    }

    let num = match args[0] {
        Value::Number(n) => n as i64,
        _ => {
            return Err(VmError::RuntimeError(
                "hex() argument must be an integer.".into(),
            ))
        }
    };

    // Handle negative numbers like Python does
    if num < 0 {
        Ok(Value::IoString(Gc::new(
            state,
            format!("-0x{:x}", num.abs()),
        )))
    } else {
        Ok(Value::IoString(Gc::new(state, format!("0x{:x}", num))))
    }
}

pub(super) fn oct<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "oct() takes exactly one argument.".into(),
        ));
    }

    let num = match args[0] {
        Value::Number(n) => n as i64,
        _ => {
            return Err(VmError::RuntimeError(
                "oct() argument must be an integer.".into(),
            ))
        }
    };

    // Handle negative numbers like Python does
    if num < 0 {
        Ok(Value::IoString(Gc::new(
            state,
            format!("-0o{:o}", num.abs()),
        )))
    } else {
        Ok(Value::IoString(Gc::new(state, format!("0o{:o}", num))))
    }
}
