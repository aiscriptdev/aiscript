use std::collections::HashMap;

use codegen::CodeGen;
use gc_arena::Gc;
use parser::Parser;

use crate::{object::Function, vm::Context, VmError};

pub(crate) mod ast;
mod codegen;
mod lexer;
mod parser;
mod pretty;
pub(crate) mod ty;

pub fn compile<'gc>(
    ctx: Context<'gc>,
    source: &'gc str,
) -> Result<HashMap<usize, Gc<'gc, Function<'gc>>>, VmError> {
    // Step 1: Parse source into AST
    let mut parser = Parser::new(ctx, source);
    let program = parser.parse()?;
    // println!("AST: {}", program);
    // Step 2: Generate bytecode from AST
    CodeGen::generate(program, ctx).map(|chunks| {
        chunks
            .into_iter()
            .map(|(id, function)| (id, Gc::new(&ctx, function)))
            .collect()
    })
}
