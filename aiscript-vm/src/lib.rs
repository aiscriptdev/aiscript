mod ai;
mod builtins;
mod chunk;
mod compiler;
mod fuel;
mod object;
mod string;
mod string_utils;
#[cfg(feature = "v1")]
mod v1;
mod value;
mod vm;

use std::collections::HashMap;
use std::fmt::Display;

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
            Self::Nil => write!(f, ""),
            Self::Object(obj) => write!(f, "{}", serde_json::to_string(obj).unwrap()),
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
