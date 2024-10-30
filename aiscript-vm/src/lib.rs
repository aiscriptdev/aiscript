mod ai;
mod builtins;
mod chunk;
mod compiler;
mod fuel;
mod object;
mod scanner;
mod string;
mod string_utils;
mod value;
mod vm;

pub(crate) use chunk::{Chunk, OpCode};
pub use value::Value;
pub use vm::Vm;
pub use vm::VmError;

#[derive(Debug, PartialEq)]
pub enum ReturnValue {
    Number(f64),
    Boolean(bool),
    String(String),
    Nil,
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
