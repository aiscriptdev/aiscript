use std::collections::BTreeMap;

use codegen::CodeGen;
use gc_arena::Gc;

use crate::{ast::ChunkId, object::Function, parser::Parser, vm::Context, VmError};

mod codegen;
#[cfg(feature = "optimizer")]
mod optimizer;

pub fn compile<'gc>(
    ctx: Context<'gc>,
    source: &'gc str,
) -> Result<BTreeMap<ChunkId, Gc<'gc, Function<'gc>>>, VmError> {
    let mut parser = Parser::new(ctx, source);
    let program = parser.parse()?;
    #[cfg(feature = "debug")]
    println!("AST: {}", program);
    #[cfg(feature = "optimizer")]
    let optimizer = optimizer::ChunkOptimizer::new();

    CodeGen::generate(program, ctx).map(|chunks| {
        chunks
            .into_iter()
            .map(|(id, function)| {
                #[cfg(feature = "optimizer")]
                let mut function = function;
                #[cfg(feature = "optimizer")]
                optimizer.optimize(&mut function.chunk);
                (id, Gc::new(&ctx, function))
            })
            .collect()
    })
}
