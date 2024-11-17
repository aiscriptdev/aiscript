use crate::{Value, VmError};

pub fn clock(_args: Vec<Value>) -> Result<Value, VmError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    Ok(now.into())
}
