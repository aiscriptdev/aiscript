use aiscript_arena::{Gc, RefLock};

use crate::{
    object::{Class, Instance, Object},
    vm::State,
    Value, VmError,
};

/// Creates a new Response instance with the given status code, body, headers, and cookies.
/// Usage: response(status_code=200, body={}, headers={}, cookies={})
pub fn response<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    // Initialize default values
    let mut status_code = 200.0;
    let mut body = Value::Nil; // body can be any Value
    let mut headers = Value::Object(Gc::new(state, RefLock::default()));
    let mut cookies = Value::Object(Gc::new(state, RefLock::default()));

    // Parse and validate keyword arguments in a single pass
    let mut i = 0;
    while i < args.len() {
        match &args[i] {
            Value::String(key) if i + 1 < args.len() => match key.to_str().unwrap() {
                "status_code" => {
                    status_code = args[i + 1].as_number().map_err(|_| {
                        VmError::RuntimeError("status_code must be a number".into())
                    })?;
                    i += 2;
                }
                "body" => {
                    body = args[i + 1];
                    i += 2;
                }
                "headers" => {
                    let value = args[i + 1];
                    if !value.is_object() {
                        return Err(VmError::RuntimeError("headers must be an object".into()));
                    }
                    headers = value;
                    i += 2;
                }
                "cookies" => {
                    let value = args[i + 1];
                    if !value.is_object() {
                        return Err(VmError::RuntimeError("cookies must be an object".into()));
                    }
                    cookies = value;
                    i += 2;
                }
                _ => {
                    return Err(VmError::RuntimeError(format!(
                        "Unknown keyword argument: {}",
                        key
                    )));
                }
            },
            _ => {
                return Err(VmError::RuntimeError(
                    "Response() requires keyword arguments (e.g., Response(status_code=200, body=\"hello\"))".into(),
                ));
            }
        }
    }

    // Validate status code
    if !(100.0..=599.0).contains(&status_code) {
        return Err(VmError::RuntimeError(
            "status_code must be between 100 and 599".into(),
        ));
    }

    // Create Response instance
    let class = Class::new(state.intern(b"Response"));
    let mut instance = Instance::new(Gc::new(state, RefLock::new(class)));
    instance.fields = [
        (state.intern(b"status_code"), Value::Number(status_code)),
        (state.intern(b"body"), body),
        (state.intern(b"headers"), headers),
        (state.intern(b"cookies"), cookies),
    ]
    .into_iter()
    .collect();

    Ok(Value::Instance(Gc::new(state, RefLock::new(instance))))
}

/// Creates a temporary redirect (307) response with the specified target URL
/// Usage: temporary_redirect(target="/new-url")
pub fn temporary_redirect<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 2 || !matches!(args[0], Value::String(_)) {
        return Err(VmError::RuntimeError(
            "temporary_redirect() requires a target URL as keyword argument (e.g., temporary_redirect(target=\"/new-url\"))".into()
        ));
    }

    if args[0].as_string()?.to_str().unwrap() != "target" {
        return Err(VmError::RuntimeError(
            "temporary_redirect() requires 'target' keyword argument".into(),
        ));
    }

    let target = args[1].as_string()?;

    // Create headers with Location set
    let fields = [(state.intern(b"Location"), Value::String(target))]
        .into_iter()
        .collect();
    // Create response with 307 status code
    let args = [
        Value::String(state.intern(b"status_code")),
        Value::Number(307.0),
        Value::String(state.intern(b"headers")),
        Value::Object(Gc::new(state, RefLock::new(Object { fields }))),
    ]
    .into_iter()
    .collect();
    response(state, args)
}

/// Creates a permanent redirect (308) response with the specified target URL
/// Usage: permanent_redirect(target="/new-url")
pub fn permanent_redirect<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 2 || !matches!(args[0], Value::String(_)) {
        return Err(VmError::RuntimeError(
            "permanent_redirect() requires a target URL as keyword argument (e.g., permanent_redirect(target=\"/new-url\"))".into()
        ));
    }

    if args[0].as_string()?.to_str().unwrap() != "target" {
        return Err(VmError::RuntimeError(
            "permanent_redirect() requires 'target' keyword argument".into(),
        ));
    }

    let target = args[1].as_string()?;

    // Create headers with Location set
    let fields = [(state.intern(b"Location"), Value::String(target))]
        .into_iter()
        .collect();

    // Create response with 308 status code
    let args = [
        Value::String(state.intern(b"status_code")),
        Value::Number(308.0),
        Value::String(state.intern(b"headers")),
        Value::Object(Gc::new(state, RefLock::new(Object { fields }))),
    ]
    .into_iter()
    .collect();

    response(state, args)
}
