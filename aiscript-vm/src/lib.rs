mod agent;
mod ai;
#[cfg(not(feature = "v1"))]
mod ast;
mod builtins;
mod chunk;
#[cfg(not(feature = "v1"))]
mod codegen;
mod fuel;
mod lexer;
mod object;
#[cfg(not(feature = "v1"))]
mod parser;
#[cfg(not(feature = "v1"))]
mod pretty;
mod string;
mod string_utils;
#[cfg(feature = "v1")]
mod v1;
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
