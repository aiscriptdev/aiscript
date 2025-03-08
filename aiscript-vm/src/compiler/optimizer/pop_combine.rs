use crate::{OpCode, chunk::Chunk};
use std::collections::HashMap;

use super::OptimizationStrategy;

/// Combines consecutive POP instructions where possible
pub(super) struct PopCombiner;

impl OptimizationStrategy for PopCombiner {
    fn optimize(&self, chunk: &mut Chunk) -> bool {
        let mut modified = false;
        let mut i = 0;

        // Map to track jump targets
        let mut jump_targets: HashMap<usize, bool> = HashMap::new();

        // First pass: collect all jump targets
        for (pos, op) in chunk.code.iter().enumerate() {
            match op {
                OpCode::Jump(offset)
                | OpCode::JumpIfFalse(offset)
                | OpCode::JumpIfError(offset) => {
                    let target = pos + *offset as usize;
                    jump_targets.insert(target, true);
                }
                OpCode::Loop(offset) => {
                    // Handle loop target calculation safely
                    if let Some(target) = pos.checked_sub(*offset as usize) {
                        if let Some(target) = target.checked_add(1) {
                            jump_targets.insert(target, true);
                        }
                    }
                }
                _ => {}
            }
        }

        while i < chunk.code.len() {
            // Skip if current position is a jump target
            if jump_targets.contains_key(&i) {
                i += 1;
                continue;
            }

            if let OpCode::Pop(count) = chunk.code[i] {
                let mut total_count = count;
                let start_pos = i;
                let mut next_pos = i + 1;

                // Look ahead for consecutive POPs
                while next_pos < chunk.code.len() && !jump_targets.contains_key(&next_pos) {
                    match chunk.code[next_pos] {
                        OpCode::Pop(next_count) => {
                            // Check the next instruction after POP
                            if next_pos + 1 < chunk.code.len() {
                                match chunk.code[next_pos + 1] {
                                    // Don't combine POPs before these instructions
                                    OpCode::CloseUpvalue | OpCode::Loop(_) => break,
                                    _ => {
                                        total_count += next_count;
                                        next_pos += 1;
                                    }
                                }
                            } else {
                                total_count += next_count;
                                next_pos += 1;
                            }
                        }
                        _ => break,
                    }
                }

                // If we found consecutive POPs
                if next_pos > start_pos + 1 {
                    // Replace multiple POPs with a single POP
                    chunk.code[start_pos] = OpCode::Pop(total_count);

                    // Remove the other POP instructions
                    chunk.code.drain(start_pos + 1..next_pos);

                    // Adjust jump offsets for any jumps that cross the removed region
                    for (pos, op) in chunk.code.iter_mut().enumerate() {
                        match op {
                            OpCode::Jump(offset)
                            | OpCode::JumpIfFalse(offset)
                            | OpCode::JumpIfError(offset) => {
                                let target = pos + *offset as usize;
                                if pos < start_pos && target > start_pos {
                                    *offset -= (next_pos - start_pos - 1) as u16;
                                } else if pos > next_pos && target < start_pos {
                                    *offset += (next_pos - start_pos - 1) as u16;
                                }
                            }
                            OpCode::Loop(offset) => {
                                let target = pos - *offset as usize + 1;
                                if pos > next_pos && target < start_pos {
                                    *offset -= (next_pos - start_pos - 1) as u16;
                                } else if pos < start_pos && target > start_pos {
                                    *offset += (next_pos - start_pos - 1) as u16;
                                }
                            }
                            _ => {}
                        }
                    }

                    modified = true;
                    // Stay at current position to check for more optimizations
                    continue;
                }
            }
            i += 1;
        }

        modified
    }
}

#[cfg(test)]
mod tests {
    use crate::compiler::optimizer::ChunkOptimizer;

    use super::*;

    #[test]
    fn test_pop_combiner() {
        let mut chunk = Chunk::new();
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Pop(1), 1);

        let optimizer = ChunkOptimizer::new();
        optimizer.optimize(&mut chunk);

        assert_eq!(chunk.code.len(), 1);
        assert_eq!(chunk.code[0], OpCode::Pop(3));
    }

    #[test]
    fn test_pop_combiner_with_jumps() {
        let mut chunk = Chunk::new();
        // Create a sequence with a jump in the middle
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::JumpIfFalse(2), 1); // Jump target points after next instruction
        chunk.write_code(OpCode::Pop(1), 1); // This can't be combined with anything due to the jump
        chunk.write_code(OpCode::Pop(1), 1); // This can be combined with previous Pop

        let optimizer = ChunkOptimizer::new();
        optimizer.optimize(&mut chunk);

        // The final chunk should have 3 instructions:
        // 1. First Pop(1) before jump
        // 2. The jump instruction
        // 3. A Pop(1) at the jump target (can't combine due to being a jump target)
        // 4. Another Pop(1) that couldn't be combined due to jump target
        assert_eq!(chunk.code.len(), 4);
        assert_eq!(chunk.code[0], OpCode::Pop(1)); // First Pop stays as is
        assert_eq!(chunk.code[2], OpCode::Pop(1)); // Target of jump must stay separate
        assert_eq!(chunk.code[3], OpCode::Pop(1)); // Last Pop stays separate too
    }

    #[test]
    fn test_pop_combiner_simple() {
        let mut chunk = Chunk::new();
        // Simple sequence of Pops with no jumps
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Pop(1), 1);

        let optimizer = ChunkOptimizer::new();
        optimizer.optimize(&mut chunk);

        // Should combine all three Pops into one
        assert_eq!(chunk.code.len(), 1);
        assert_eq!(chunk.code[0], OpCode::Pop(3));
    }

    #[test]
    fn test_pop_combiner_with_other_ops() {
        let mut chunk = Chunk::new();
        // Pops separated by other operations
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Nil, 1); // Different operation
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Pop(1), 1);

        let optimizer = ChunkOptimizer::new();
        optimizer.optimize(&mut chunk);

        // Should combine Pops only up to the Nil operation
        assert_eq!(chunk.code.len(), 3);
        assert_eq!(chunk.code[0], OpCode::Pop(2)); // First two Pops combined
        assert_eq!(chunk.code[1], OpCode::Nil); // Nil operation remains
        assert_eq!(chunk.code[2], OpCode::Pop(2)); // Last two Pops combined
    }

    #[test]
    fn test_pop_combiner_with_upvalue_closure() {
        let mut chunk = Chunk::new();
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::CloseUpvalue, 1);
        chunk.write_code(OpCode::Pop(1), 1);

        let optimizer = ChunkOptimizer::new();
        optimizer.optimize(&mut chunk);

        // POPs before CloseUpvalue should not be combined
        assert_eq!(chunk.code.len(), 4);
        assert_eq!(chunk.code[0], OpCode::Pop(1));
        assert_eq!(chunk.code[1], OpCode::Pop(1));
        assert_eq!(chunk.code[2], OpCode::CloseUpvalue);
        assert_eq!(chunk.code[3], OpCode::Pop(1));
    }

    #[test]
    fn test_pop_combiner_with_loop_locals() {
        let mut chunk = Chunk::new();
        // Simulate a loop with local variables
        chunk.write_code(OpCode::Pop(1), 1); // Pop first local
        chunk.write_code(OpCode::Pop(1), 1); // Pop second local
        chunk.write_code(OpCode::Loop(5), 1); // Loop back

        let optimizer = ChunkOptimizer::new();
        optimizer.optimize(&mut chunk);

        // POPs before Loop should not be combined
        assert_eq!(chunk.code.len(), 3);
        assert_eq!(chunk.code[0], OpCode::Pop(1));
        assert_eq!(chunk.code[1], OpCode::Pop(1));
        assert_eq!(chunk.code[2], OpCode::Loop(5));
    }

    #[test]
    fn test_pop_combiner_with_infinite_loop() {
        let mut chunk = Chunk::new();
        // Create a simple infinite loop
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Pop(1), 1);
        chunk.write_code(OpCode::Loop(2), 1); // Loop back 2 positions
        chunk.write_code(OpCode::Pop(1), 1);

        let optimizer = ChunkOptimizer::new();
        optimizer.optimize(&mut chunk);

        // All POPs should remain separate because:
        // 1. First POP might be a loop target
        // 2. Second POP is right before Loop so it's referenced by loop's offset
        // 3. Third POP is after Loop
        assert_eq!(chunk.code.len(), 4);
        assert_eq!(chunk.code[0], OpCode::Pop(1)); // Can't combine - potential loop target
        assert_eq!(chunk.code[1], OpCode::Pop(1)); // Can't combine - referenced by loop offset
        assert_eq!(chunk.code[2], OpCode::Loop(2)); // Loop instruction
        assert_eq!(chunk.code[3], OpCode::Pop(1)); // Can't combine - after loop
    }
}
