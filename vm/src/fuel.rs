pub struct Fuel {
    value: i32,
}

impl Fuel {
    pub fn new(value: i32) -> Self {
        Fuel { value }
    }

    /// Add to or subtract from the current remaining fuel.
    pub fn adjust(&mut self, fuel: i32) {
        self.value = self.value.saturating_add(fuel);
    }

    /// Subtract from the current remaining fuel.
    ///
    /// This is a convenience method that is equivalent to `self.adjust_fuel(-fuel)`.
    pub fn consume(&mut self, fuel: i32) {
        self.adjust(fuel.saturating_neg());
    }

    /// Returns true if we have positive fuel remaining.
    pub fn should_continue(&self) -> bool {
        self.value > 0
    }
}
