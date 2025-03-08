use super::OptimizationStrategy;
use crate::{OpCode, chunk::Chunk};
use std::collections::HashMap;

// Removes unreachable code after unconditional control flow
pub(super) struct DeadCodeEliminator;

impl OptimizationStrategy for DeadCodeEliminator {
    fn optimize(&self, chunk: &mut Chunk) -> bool {
        self.eliminate_dead_code(chunk)
    }
}

impl DeadCodeEliminator {
    // Get all positions that are targets of jumps
    fn collect_jump_targets(chunk: &Chunk) -> HashMap<usize, bool> {
        let mut jump_targets = HashMap::new();

        for (pos, op) in chunk.code.iter().enumerate() {
            match op {
                OpCode::Jump(offset)
                | OpCode::JumpIfFalse(offset)
                | OpCode::JumpIfError(offset)
                | OpCode::JumpPopIfFalse(offset) => {
                    let target = pos + *offset as usize;
                    jump_targets.insert(target, true);
                }
                OpCode::Loop(offset) => {
                    if let Some(target) = pos.checked_sub(*offset as usize) {
                        if let Some(target) = target.checked_add(1) {
                            jump_targets.insert(target, true);
                        }
                    }
                }
                _ => {}
            }
        }

        jump_targets
    }

    // Get regions between conditional jumps and their targets - these must be preserved
    fn collect_conditional_regions(chunk: &Chunk) -> Vec<(usize, usize)> {
        let mut regions = Vec::new();
        for (pos, op) in chunk.code.iter().enumerate() {
            match op {
                OpCode::JumpIfFalse(offset) | OpCode::JumpPopIfFalse(offset) => {
                    let target = pos + *offset as usize;
                    regions.push((pos, target));
                }
                _ => {}
            }
        }
        regions
    }

    fn is_in_conditional_region(pos: usize, regions: &[(usize, usize)]) -> bool {
        regions
            .iter()
            .any(|&(start, end)| pos >= start && pos <= end)
    }

    fn eliminate_dead_code(&self, chunk: &mut Chunk) -> bool {
        let mut modified = false;
        let jump_targets = Self::collect_jump_targets(chunk);
        let conditional_regions = Self::collect_conditional_regions(chunk);

        // Process instructions
        let mut i = 0;
        while i < chunk.code.len() {
            // Only eliminate after real unconditional jumps
            if let OpCode::Jump(_) = chunk.code[i] {
                let dead_start = i + 1;

                // Instructions from jump+1 to target are potentially dead
                if dead_start < chunk.code.len() {
                    // Check if this section is safe to eliminate
                    let mut can_eliminate = true;
                    let mut dead_end = dead_start;

                    while dead_end < chunk.code.len() {
                        // Can't remove if it's a jump target or in a conditional region
                        if jump_targets.contains_key(&dead_end)
                            || Self::is_in_conditional_region(dead_end, &conditional_regions)
                        {
                            can_eliminate = false;
                            break;
                        }

                        // Can't remove control flow instructions
                        match chunk.code[dead_end] {
                            OpCode::Return
                            | OpCode::Jump(_)
                            | OpCode::JumpIfFalse(_)
                            | OpCode::JumpIfError(_)
                            | OpCode::JumpPopIfFalse(_)
                            | OpCode::Loop(_)
                            | OpCode::CloseUpvalue => {
                                can_eliminate = false;
                                break;
                            }
                            _ => dead_end += 1,
                        }
                    }

                    // Remove dead code if safe
                    if can_eliminate && dead_end > dead_start {
                        let count = dead_end - dead_start;
                        chunk.code.drain(dead_start..dead_end);
                        chunk.lines.drain(dead_start..dead_end);

                        // Adjust jump offsets
                        for (pos, op) in chunk.code.iter_mut().enumerate() {
                            match op {
                                OpCode::Jump(ref mut jump_offset)
                                | OpCode::JumpIfFalse(ref mut jump_offset)
                                | OpCode::JumpIfError(ref mut jump_offset)
                                | OpCode::JumpPopIfFalse(ref mut jump_offset) => {
                                    let target = pos + *jump_offset as usize;
                                    if pos < dead_start && target > dead_start {
                                        *jump_offset -= count as u16;
                                    }
                                }
                                OpCode::Loop(ref mut jump_offset) => {
                                    if let Some(target) = pos.checked_sub(*jump_offset as usize) {
                                        if pos > dead_end && target < dead_start {
                                            *jump_offset -= count as u16;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }

                        modified = true;
                        continue;
                    }
                }
            }
            i += 1;
        }

        modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_if_nil() {
        let mut chunk = Chunk::new();
        // if nil { print("bad"); } else { print("nil"); }
        chunk.write_code(OpCode::Nil, 1); // condition
        chunk.write_code(OpCode::JumpPopIfFalse(4), 1); // skip true branch
        chunk.write_code(OpCode::GetGlobal(0), 1); // true: print
        chunk.write_code(OpCode::Constant(1), 1); // "bad"
        chunk.write_code(
            OpCode::Call {
                positional_count: 1,
                keyword_count: 0,
            },
            1,
        ); // print("bad")
        chunk.write_code(OpCode::Pop(1), 1); // cleanup
        chunk.write_code(OpCode::Jump(1), 1); // skip else
        chunk.write_code(OpCode::Pop(1), 1); // else: cleanup
        chunk.write_code(OpCode::Nil, 1); // else: return nil
        chunk.write_code(OpCode::Return, 1); // return

        let optimizer = DeadCodeEliminator;
        assert!(!optimizer.optimize(&mut chunk)); // Should not optimize conditional code
        assert_eq!(chunk.code.len(), 10); // Everything should be preserved
    }

    // #[test]
    // fn test_unconditional_dead_code() {
    //     let mut chunk = Chunk::new();
    //     chunk.write_code(OpCode::Jump(3), 1); // Unconditional jump
    //     chunk.write_code(OpCode::Pop(1), 1); // Dead code
    //     chunk.write_code(OpCode::Pop(1), 1); // Dead code
    //     chunk.write_code(OpCode::Return, 1); // Live code

    //     let optimizer = DeadCodeEliminator;
    //     assert!(optimizer.optimize(&mut chunk));
    //     assert_eq!(chunk.code.len(), 2);
    // }

    #[test]
    fn test_preserve_error_handler() {
        let mut chunk = Chunk::new();
        chunk.write_code(OpCode::Jump(3), 1); // Jump
        chunk.write_code(OpCode::JumpIfError(2), 1); // Error handler
        chunk.write_code(OpCode::Pop(1), 1); // Error handling
        chunk.write_code(OpCode::Return, 1); // Return

        let optimizer = DeadCodeEliminator;
        assert!(!optimizer.optimize(&mut chunk));
        assert_eq!(chunk.code.len(), 4);
    }
}
