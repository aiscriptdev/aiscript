mod ai;
mod ast;
mod builtins;
mod chunk;
mod codegen;
mod compiler;
mod fuel;
mod object;
mod parser;
mod pretty;
mod scanner;
mod string;
mod string_utils;
mod value;
mod vm;

use std::collections::HashMap;

pub(crate) use chunk::{Chunk, OpCode};
pub use value::Value;
pub use vm::Vm;
pub use vm::VmError;

#[derive(Debug, PartialEq)]
pub enum ReturnValue {
    Number(f64),
    Boolean(bool),
    String(String),
    Object(HashMap<String, serde_json::Value>),
    Nil,
}

impl ReturnValue {
    pub fn as_object(&self) -> Option<HashMap<String, serde_json::Value>> {
        match self {
            ReturnValue::Object(obj) => Some(obj.clone()),
            _ => None,
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
