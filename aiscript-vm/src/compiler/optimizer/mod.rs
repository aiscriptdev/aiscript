use crate::chunk::Chunk;

mod dead_code;
mod pop_combine;

/// Defines a single optimization strategy
pub(super) trait OptimizationStrategy {
    /// Apply the optimization strategy to the given chunk
    /// Returns true if any changes were made
    fn optimize(&self, chunk: &mut Chunk) -> bool;
}

/// The main optimizer that applies multiple optimization strategies
pub(super) struct ChunkOptimizer {
    strategies: Vec<Box<dyn OptimizationStrategy>>,
}

impl ChunkOptimizer {
    pub fn new() -> Self {
        let mut optimizer = ChunkOptimizer {
            strategies: Vec::new(),
        };

        // Add default optimization strategies
        optimizer.add_strategy(Box::new(pop_combine::PopCombiner));
        optimizer.add_strategy(Box::new(dead_code::DeadCodeEliminator));

        optimizer
    }

    /// Add a new optimization strategy
    pub fn add_strategy(&mut self, strategy: Box<dyn OptimizationStrategy>) {
        self.strategies.push(strategy);
    }

    /// Optimize the given chunk using all registered strategies
    pub fn optimize(&self, chunk: &mut Chunk) {
        let mut modified = true;
        let mut iteration = 0;
        const MAX_ITERATIONS: usize = 10; // Prevent infinite loops

        while modified && iteration < MAX_ITERATIONS {
            modified = false;
            for strategy in &self.strategies {
                if strategy.optimize(chunk) {
                    modified = true;
                }
            }
            iteration += 1;
        }
    }
}
