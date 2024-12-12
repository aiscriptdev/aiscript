mod ai;
mod ast;
mod builtins;
mod chunk;
mod compiler;
mod module;
mod object;
mod parser;
mod stdlib;
mod string;
mod ty;
mod value;
mod vm;

use std::collections::HashMap;
use std::fmt::Display;
use std::ops::Deref;

pub(crate) use aiscript_lexer as lexer;
pub(crate) use chunk::{Chunk, OpCode};
use gc_arena::Collect;
use gc_arena::Mutation;
pub use value::Value;
use vm::State;
pub use vm::Vm;
pub use vm::VmError;

type NativeFnInner<'gc> = fn(&mut State<'gc>, Vec<Value<'gc>>) -> Result<Value<'gc>, VmError>;
type BuiltinMethodInner<'gc> = fn(
    &'gc Mutation<'gc>,
    // receiver
    Value<'gc>,
    // args
    Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError>;

#[derive(Debug, Clone, Copy)]
pub struct NativeFn<'gc>(NativeFnInner<'gc>);

#[derive(Debug, Clone, Copy)]
pub struct BuiltinMethod<'gc>(BuiltinMethodInner<'gc>);

impl<'gc> Deref for NativeFn<'gc> {
    type Target = NativeFnInner<'gc>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'gc> Deref for BuiltinMethod<'gc> {
    type Target = BuiltinMethodInner<'gc>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

unsafe impl<'gc> Collect for NativeFn<'gc> {
    fn needs_trace() -> bool
    where
        Self: Sized,
    {
        false
    }

    fn trace(&self, _cc: &gc_arena::Collection) {}
}

unsafe impl<'gc> Collect for BuiltinMethod<'gc> {
    fn needs_trace() -> bool
    where
        Self: Sized,
    {
        false
    }

    fn trace(&self, _cc: &gc_arena::Collection) {}
}

#[derive(Debug, PartialEq)]
pub enum ReturnValue {
    Number(f64),
    Boolean(bool),
    String(String),
    Object(HashMap<String, serde_json::Value>),
    Agent(String), // agent name
    Nil,
}

impl ReturnValue {
    pub fn as_object(&self) -> Option<HashMap<String, serde_json::Value>> {
        match self {
            Self::Object(obj) => Some(obj.clone()),
            _ => None,
        }
    }
}

impl Display for ReturnValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(s) => write!(f, "{s}"),
            Self::Number(n) => write!(f, "{n}"),
            Self::Boolean(b) => write!(f, "{b}"),
            Self::Agent(name) => write!(f, "{name}"),
            Self::Object(obj) => write!(f, "{}", serde_json::to_string(obj).unwrap()),
            Self::Nil => write!(f, ""),
        }
    }
}

pub fn eval(source: &'static str) -> Result<ReturnValue, VmError> {
    let mut vm = Vm::new();
    vm.compile(source)?;
    vm.interpret()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_expression() {
        assert_eq!(eval("return 1 + 2 * 3;").unwrap(), ReturnValue::Number(7.0));
    }
}
