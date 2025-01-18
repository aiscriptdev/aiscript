use crate::{
    object::{Class, Object},
    string::InternedString,
    value::Value,
    vm::Context,
};
use gc_arena::{Gc, GcRefLock, RefLock};
use std::collections::HashMap;

pub fn create_validation_error(ctx: Context) -> GcRefLock<'_, Class> {
    let error_class = Class::new(ctx.intern(b"ValidationError!"));
    Gc::new(&ctx, RefLock::new(error_class))
}

// Helper to create error info object
pub fn create_error_info<'gc>(
    ctx: Context<'gc>,
    field: InternedString<'gc>,
    error_type: &str,
    message: &str,
    input: Value<'gc>,
) -> Value<'gc> {
    let mut fields = HashMap::default();
    fields.insert(
        ctx.intern(b"type"),
        Value::String(ctx.intern(error_type.as_bytes())),
    );
    fields.insert(
        ctx.intern(b"loc"),
        Value::array(&ctx, vec![Value::String(field)]),
    );
    fields.insert(
        ctx.intern(b"msg"),
        Value::String(ctx.intern(message.as_bytes())),
    );
    fields.insert(ctx.intern(b"input"), input);

    Value::Object(Gc::new(&ctx, RefLock::new(Object { fields })))
}
