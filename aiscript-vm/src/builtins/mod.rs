use crate::{
    string::InternedString,
    vm::{Context, State},
    BuiltinMethod, NativeFn, Value, VmError,
};
use aiscript_arena::{Collect, Gc, Mutation};
use std::{
    collections::HashMap,
    io::{self, Write},
};

mod convert;
mod error;
mod format;
mod function;
mod print;
pub(crate) mod response;
pub(crate) mod sso;
mod string;

use convert::*;
pub use error::*;
use format::format;
use function::*;
use print::print;

#[derive(Collect)]
#[collect(no_drop)]
pub(crate) struct BuiltinMethods<'gc> {
    string: HashMap<InternedString<'gc>, BuiltinMethod<'gc>>,
}

impl Default for BuiltinMethods<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'gc> BuiltinMethods<'gc> {
    pub fn new() -> Self {
        BuiltinMethods {
            string: HashMap::default(),
        }
    }

    pub fn init(&mut self, ctx: Context<'gc>) {
        self.string = string::define_string_methods(ctx);
    }

    pub fn invoke_string_method(
        &self,
        mc: &'gc Mutation<'gc>,
        name: InternedString<'gc>,
        receiver: Value<'gc>,
        args: Vec<Value<'gc>>,
    ) -> Result<Value<'gc>, VmError> {
        if let Some(f) = self.string.get(&name) {
            f(mc, receiver, args)
        } else {
            Err(VmError::RuntimeError(format!(
                "Unknown string method: {}",
                name
            )))
        }
    }
}

pub(crate) fn define_builtin_functions(state: &mut State) {
    [
        ("abs", NativeFn(abs)),
        ("all", NativeFn(all)),
        ("any", NativeFn(any)),
        ("ascii", NativeFn(ascii)),
        ("bin", NativeFn(bin)),
        ("bool", NativeFn(bool)),
        ("callable", NativeFn(callable)),
        ("chr", NativeFn(chr)),
        ("filter", NativeFn(filter)),
        ("float", NativeFn(float)),
        ("format", NativeFn(format)),
        ("hex", NativeFn(hex)),
        ("input", NativeFn(input)),
        ("int", NativeFn(int)),
        ("len", NativeFn(len)),
        ("map", NativeFn(map)),
        ("max", NativeFn(max)),
        ("min", NativeFn(min)),
        ("oct", NativeFn(oct)),
        ("ord", NativeFn(ord)),
        ("print", NativeFn(print)),
        ("round", NativeFn(round)),
        ("str", NativeFn(str)),
        ("sum", NativeFn(sum)),
        ("zip", NativeFn(zip)),
    ]
    .into_iter()
    .for_each(|(name, func)| state.define_native_function(name, func));
}

fn abs<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "abs() takes exactly one argument.".into(),
        ));
    }

    match args[0] {
        Value::Number(n) => Ok(n.abs().into()),
        _ => Err(VmError::RuntimeError(
            "abs() argument must be a number.".into(),
        )),
    }
}

fn len<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "len() takes exactly one argument.".into(),
        ));
    }

    match &args[0] {
        Value::String(s) => Ok(Value::Number(s.len() as f64)),
        Value::IoString(s) => Ok(Value::Number(s.len() as f64)),
        Value::List(arr) => Ok(Value::Number(arr.borrow().data.len() as f64)),
        Value::Object(obj) => Ok(Value::Number(obj.borrow().fields.len() as f64)),
        _ => Err(VmError::RuntimeError(
            "len() argument must be a string, array or object.".into(),
        )),
    }
}

fn any<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "any() takes exactly one argument.".into(),
        ));
    }

    match &args[0] {
        Value::List(arr) => {
            let arr = &arr.borrow().data;
            Ok(Value::Boolean(arr.iter().any(|x| x.is_true())))
        }
        _ => Err(VmError::RuntimeError(
            "any() argument must be an array.".into(),
        )),
    }
}

fn all<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "all() takes exactly one argument.".into(),
        ));
    }

    match &args[0] {
        Value::List(arr) => {
            let arr = arr.borrow();
            Ok(Value::Boolean(arr.data.iter().all(|x| x.is_true())))
        }
        _ => Err(VmError::RuntimeError(
            "all() argument must be an array.".into(),
        )),
    }
}

fn min<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "min() takes at least one argument.".into(),
        ));
    }

    if args.len() == 1 {
        // If single argument, it should be an array
        match &args[0] {
            Value::List(arr) => {
                let arr = &arr.borrow().data;
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
                "single argument to min() must be an array.".into(),
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
            "max() takes at least one argument.".into(),
        ));
    }

    if args.len() == 1 {
        // If single argument, it should be an array
        match &args[0] {
            Value::List(arr) => {
                let arr = &arr.borrow().data;
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
                "single argument to max() must be an array.".into(),
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
            "round() takes exactly one argument.".into(),
        ));
    }

    match args[0] {
        Value::Number(n) => Ok(n.round().into()),
        _ => Err(VmError::RuntimeError(
            "round() argument must be a number.".into(),
        )),
    }
}

fn sum<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "sum() takes exactly one argument.".into(),
        ));
    }

    match &args[0] {
        Value::List(arr) => {
            let arr = &arr.borrow().data;
            let mut sum = 0.0;
            for value in arr.iter() {
                if let Value::Number(n) = value {
                    sum += n;
                } else {
                    return Err(VmError::RuntimeError(
                        "sum() array elements must be numbers.".into(),
                    ));
                }
            }
            Ok(sum.into())
        }
        _ => Err(VmError::RuntimeError(
            "sum() argument must be an array.".into(),
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
                    "input() prompt must be a string.".into(),
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
            "callable() takes exactly one argument.".into(),
        ));
    }

    Ok(Value::Boolean(matches!(
        args[0],
        Value::Closure(_) | Value::NativeFunction(_) | Value::BoundMethod(_) | Value::Class(_)
    )))
}
