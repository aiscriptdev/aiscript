use crate::Value;

pub fn clock(_args: Vec<Value>) -> Value {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    now.into()
}
