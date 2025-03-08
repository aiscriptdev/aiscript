mod interned;
mod utils;

use aiscript_arena::Gc;
pub use interned::{InternedString, InternedStringSet};

use crate::{Value, vm::Context};

/// Internal enum to handle string operations
pub enum StringValue<'gc> {
    Interned(InternedString<'gc>),
    Dynamic(Gc<'gc, String>),
}

impl<'gc> StringValue<'gc> {
    pub fn as_str(&self) -> &str {
        match self {
            StringValue::Interned(s) => s.to_str().unwrap(),
            StringValue::Dynamic(s) => s,
        }
    }

    // Convert back to Value based on heuristics
    pub fn into_value(self, ctx: Context<'gc>, should_intern: bool) -> Value<'gc> {
        match (self, should_intern) {
            (StringValue::Interned(s), _) => Value::String(s),
            (StringValue::Dynamic(s), true) => Value::String(ctx.intern(s.as_bytes())),
            (StringValue::Dynamic(s), false) => Value::IoString(s),
        }
    }
}
