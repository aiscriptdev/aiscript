use gc_arena::{Gc, RefLock};

use crate::{
    NativeFn, Value, VmError,
    module::ModuleKind,
    object::Object,
    vm::{Context, State},
};

pub fn create_env_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.env");

    let exports = [
        ("args", Value::NativeFunction(NativeFn(env_args))),
        ("vars", Value::NativeFunction(NativeFn(env_vars))),
        ("get_env", Value::NativeFunction(NativeFn(env_get))),
        ("set_env", Value::NativeFunction(NativeFn(env_set))),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect();

    ModuleKind::Native { name, exports }
}

// Returns command-line arguments as an array of strings
fn env_args<'gc>(state: &mut State<'gc>, _args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let ctx = state.get_context();

    let args: Vec<Value<'gc>> = std::env::args()
        .map(|arg| Value::String(ctx.intern(arg.as_bytes())))
        .collect();

    Ok(Value::array(&ctx, args))
}

// Returns all environment variables as an object
fn env_vars<'gc>(state: &mut State<'gc>, _args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let ctx = state.get_context();
    let mut env_vars = Object::default();

    for (key, value) in std::env::vars() {
        let key = ctx.intern(key.as_bytes());
        let value = Value::String(ctx.intern(value.as_bytes()));
        env_vars.fields.insert(key, value);
    }

    Ok(Value::Object(Gc::new(&ctx, RefLock::new(env_vars))))
}

// Gets the value of an environment variable
fn env_get<'gc>(state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "get_env() requires a variable name as argument".into(),
        ));
    }

    let var_name = args[0].as_string()?.to_str().unwrap();

    match std::env::var(var_name) {
        Ok(value) => Ok(Value::String(state.get_context().intern(value.as_bytes()))),
        Err(_) => Ok(Value::Nil),
    }
}

// Sets the value of an environment variable
fn env_set<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.len() != 2 {
        return Err(VmError::RuntimeError(
            "set_env() requires two arguments: variable name and value".into(),
        ));
    }

    let var_name = args[0].as_string()?.to_str().unwrap();
    let value = args[1].as_string()?.to_str().unwrap();

    unsafe { std::env::set_var(var_name, value) };
    Ok(Value::Nil)
}
