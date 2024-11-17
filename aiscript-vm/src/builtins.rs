use gc_arena::Mutation;

use crate::{Value, VmError};

pub fn clock<'gc>(_mc: &'gc Mutation<'gc>, _args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    Ok(now.into())
}
