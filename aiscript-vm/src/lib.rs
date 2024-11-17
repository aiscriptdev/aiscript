mod ai;
mod ast;
mod builtins;
mod chunk;
mod compiler;
mod lexer;
mod module;
mod object;
mod parser;
mod stdlib;
mod string;
mod string_utils;
mod ty;
#[cfg(feature = "v1")]
mod v1;
mod value;
mod vm;

use std::collections::HashMap;
use std::fmt::Display;

pub(crate) use chunk::{Chunk, OpCode};
use gc_arena::Mutation;
pub use value::Value;
pub use vm::Vm;
pub use vm::VmError;

pub type NativeFn<'gc> = fn(&'gc Mutation<'gc>, Vec<Value<'gc>>) -> Result<Value<'gc>, VmError>;

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
