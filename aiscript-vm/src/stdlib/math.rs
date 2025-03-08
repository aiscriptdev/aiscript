use crate::{
    NativeFn, Value, VmError, float_arg,
    module::ModuleKind,
    vm::{Context, State},
};

pub fn create_math_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.math");

    let exports = [
        // Constants
        ("PI", Value::Number(std::f64::consts::PI)),
        ("E", Value::Number(std::f64::consts::E)),
        ("TAU", Value::Number(std::f64::consts::TAU)),
        ("INFINITY", Value::Number(f64::INFINITY)),
        ("NEG_INFINITY", Value::Number(f64::NEG_INFINITY)),
        ("NAN", Value::Number(f64::NAN)),
        // Functions
        ("add", Value::NativeFunction(NativeFn(math_add))),
        ("sub", Value::NativeFunction(NativeFn(math_sub))),
        ("mul", Value::NativeFunction(NativeFn(math_mul))),
        ("div", Value::NativeFunction(NativeFn(math_div))),
        ("floo_div", Value::NativeFunction(NativeFn(math_floor_div))),
        // Advanced functions
        ("sqrt", Value::NativeFunction(NativeFn(math_sqrt))),
        ("pow", Value::NativeFunction(NativeFn(math_pow))),
        ("log", Value::NativeFunction(NativeFn(math_log))),
        // Trigonometric functions
        ("sin", Value::NativeFunction(NativeFn(math_sin))),
        ("cos", Value::NativeFunction(NativeFn(math_cos))),
        ("tan", Value::NativeFunction(NativeFn(math_tan))),
        ("asin", Value::NativeFunction(NativeFn(math_asin))),
        ("acos", Value::NativeFunction(NativeFn(math_acos))),
        // Rounding functions
        ("floor", Value::NativeFunction(NativeFn(math_floor))),
        ("ceil", Value::NativeFunction(NativeFn(math_ceil))),
        ("round", Value::NativeFunction(NativeFn(math_round))),
        // Utility functions
        ("abs", Value::NativeFunction(NativeFn(math_abs))),
        ("min", Value::NativeFunction(NativeFn(math_min))),
        ("max", Value::NativeFunction(NativeFn(math_max))),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect();
    ModuleKind::Native { name, exports }
}

// Basic arithmetic functions
fn math_add<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "add")?;
    let y = float_arg!(&args, 1, "add")?;
    Ok(Value::Number(x + y))
}

fn math_sub<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "sub")?;
    let y = float_arg!(&args, 1, "sub")?;
    Ok(Value::Number(x - y))
}

fn math_mul<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "mul")?;
    let y = float_arg!(&args, 1, "mul")?;
    Ok(Value::Number(x * y))
}

fn math_div<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "div")?;
    let y = float_arg!(&args, 1, "div")?;
    if y == 0.0 {
        return Err(VmError::RuntimeError("div: division by zero".into()));
    }
    Ok(Value::Number(x / y))
}

fn math_floor_div<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "floor_div")?;
    let y = float_arg!(&args, 1, "floor_div")?;
    if y == 0.0 {
        return Err(VmError::RuntimeError("floor_div: division by zero".into()));
    }
    Ok(Value::Number((x / y).floor()))
}

fn math_sqrt<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "sqrt")?;
    if x < 0.0 {
        return Err(VmError::RuntimeError(
            "sqrt: cannot compute square root of negative number".into(),
        ));
    }
    Ok(Value::Number(x.sqrt()))
}

fn math_pow<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "pow")?;
    let y = float_arg!(&args, 1, "pow")?;

    // Handle special cases
    if x == 0.0 && y < 0.0 {
        return Err(VmError::RuntimeError(
            "pow: cannot raise zero to negative power".into(),
        ));
    }

    Ok(Value::Number(x.powf(y)))
}

fn math_log<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "log")?;
    let base = if args.len() > 1 {
        float_arg!(&args, 1, "log")?
    } else {
        std::f64::consts::E
    };

    if x <= 0.0 {
        return Err(VmError::RuntimeError(
            "log: cannot compute logarithm of non-positive number".into(),
        ));
    }
    if base <= 0.0 || base == 1.0 {
        return Err(VmError::RuntimeError(
            "log: invalid base for logarithm".into(),
        ));
    }

    Ok(Value::Number(x.log(base)))
}

fn math_sin<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "sin")?;
    Ok(Value::Number(x.sin()))
}

fn math_cos<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "cos")?;
    Ok(Value::Number(x.cos()))
}

fn math_tan<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "tan")?;
    Ok(Value::Number(x.tan()))
}

fn math_asin<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "asin")?;
    if !(-1.0..=1.0).contains(&x) {
        return Err(VmError::RuntimeError(
            "asin: argument must be between -1 and 1".into(),
        ));
    }
    Ok(Value::Number(x.asin()))
}

fn math_acos<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "acos")?;
    if !(-1.0..=1.0).contains(&x) {
        return Err(VmError::RuntimeError(
            "acos: argument must be between -1 and 1".into(),
        ));
    }
    Ok(Value::Number(x.acos()))
}

fn math_floor<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "floor")?;
    Ok(Value::Number(x.floor()))
}

fn math_ceil<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "ceil")?;
    Ok(Value::Number(x.ceil()))
}

fn math_round<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "round")?;
    Ok(Value::Number(x.round()))
}

fn math_abs<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "abs")?;
    Ok(Value::Number(x.abs()))
}

fn math_min<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "min")?;
    let y = float_arg!(&args, 1, "min")?;
    Ok(Value::Number(x.min(y)))
}

fn math_max<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "max")?;
    let y = float_arg!(&args, 1, "max")?;
    Ok(Value::Number(x.max(y)))
}
