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

pub use chunk::{Chunk, OpCode};
pub use value::Value;
use vm::Vm;

pub fn run(source: &str) -> Result<(), vm::VmError> {
    let source: &'static str = Box::leak(Box::from(source));
    let mut vm = Vm::new();
    vm.interpret(source)
}
