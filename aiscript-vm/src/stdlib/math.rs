use gc_arena::Mutation;

use crate::{float_arg, module::ModuleKind, vm::Context, Value, VmError};

pub fn create_math_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.math");

    let exports = [
        // Constants
        (ctx.intern(b"PI"), Value::Number(std::f64::consts::PI)),
        (ctx.intern(b"E"), Value::Number(std::f64::consts::E)),
        (ctx.intern(b"TAU"), Value::Number(std::f64::consts::TAU)),
        (ctx.intern(b"INFINITY"), Value::Number(f64::INFINITY)),
        (
            ctx.intern(b"NEG_INFINITY"),
            Value::Number(f64::NEG_INFINITY),
        ),
        (ctx.intern(b"NAN"), Value::Number(f64::NAN)),
        // Functions
        (ctx.intern(b"add"), Value::NativeFunction(math_add)),
        (ctx.intern(b"sub"), Value::NativeFunction(math_sub)),
        (ctx.intern(b"mul"), Value::NativeFunction(math_mul)),
        (ctx.intern(b"div"), Value::NativeFunction(math_div)),
        (
            ctx.intern(b"floo_div"),
            Value::NativeFunction(math_floor_div),
        ),
        // Advanced functions
        (ctx.intern(b"sqrt"), Value::NativeFunction(math_sqrt)),
        (ctx.intern(b"pow"), Value::NativeFunction(math_pow)),
        (ctx.intern(b"log"), Value::NativeFunction(math_log)),
        // Trigonometric functions
        (ctx.intern(b"sin"), Value::NativeFunction(math_sin)),
        (ctx.intern(b"cos"), Value::NativeFunction(math_cos)),
        (ctx.intern(b"tan"), Value::NativeFunction(math_tan)),
        (ctx.intern(b"asin"), Value::NativeFunction(math_asin)),
        (ctx.intern(b"acos"), Value::NativeFunction(math_acos)),
        // Rounding functions
        (ctx.intern(b"floor"), Value::NativeFunction(math_floor)),
        (ctx.intern(b"ceil"), Value::NativeFunction(math_ceil)),
        (ctx.intern(b"round"), Value::NativeFunction(math_round)),
        // Utility functions
        (ctx.intern(b"abs"), Value::NativeFunction(math_abs)),
        (ctx.intern(b"min"), Value::NativeFunction(math_min)),
        (ctx.intern(b"max"), Value::NativeFunction(math_max)),
    ]
    .into_iter()
    .collect();
    ModuleKind::Native { name, exports }
}

// Basic arithmetic functions
fn math_add<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "add")?;
    let y = float_arg!(&args, 1, "add")?;
    Ok(Value::Number(x + y))
}

fn math_sub<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "sub")?;
    let y = float_arg!(&args, 1, "sub")?;
    Ok(Value::Number(x - y))
}

fn math_mul<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "mul")?;
    let y = float_arg!(&args, 1, "mul")?;
    Ok(Value::Number(x * y))
}

fn math_div<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "div")?;
    let y = float_arg!(&args, 1, "div")?;
    if y == 0.0 {
        return Err(VmError::RuntimeError("div: division by zero".into()));
    }
    Ok(Value::Number(x / y))
}

fn math_floor_div<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "floor_div")?;
    let y = float_arg!(&args, 1, "floor_div")?;
    if y == 0.0 {
        return Err(VmError::RuntimeError("floor_div: division by zero".into()));
    }
    Ok(Value::Number((x / y).floor()))
}

fn math_sqrt<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "sqrt")?;
    if x < 0.0 {
        return Err(VmError::RuntimeError(
            "sqrt: cannot compute square root of negative number".into(),
        ));
    }
    Ok(Value::Number(x.sqrt()))
}

fn math_pow<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn math_log<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
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

fn math_sin<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "sin")?;
    Ok(Value::Number(x.sin()))
}

fn math_cos<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "cos")?;
    Ok(Value::Number(x.cos()))
}

fn math_tan<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "tan")?;
    Ok(Value::Number(x.tan()))
}

fn math_asin<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "asin")?;
    if !(-1.0..=1.0).contains(&x) {
        return Err(VmError::RuntimeError(
            "asin: argument must be between -1 and 1".into(),
        ));
    }
    Ok(Value::Number(x.asin()))
}

fn math_acos<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "acos")?;
    if !(-1.0..=1.0).contains(&x) {
        return Err(VmError::RuntimeError(
            "acos: argument must be between -1 and 1".into(),
        ));
    }
    Ok(Value::Number(x.acos()))
}

fn math_floor<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "floor")?;
    Ok(Value::Number(x.floor()))
}

fn math_ceil<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "ceil")?;
    Ok(Value::Number(x.ceil()))
}

fn math_round<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "round")?;
    Ok(Value::Number(x.round()))
}

fn math_abs<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "abs")?;
    Ok(Value::Number(x.abs()))
}

fn math_min<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "min")?;
    let y = float_arg!(&args, 1, "min")?;
    Ok(Value::Number(x.min(y)))
}

fn math_max<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let x = float_arg!(&args, 0, "max")?;
    let y = float_arg!(&args, 1, "max")?;
    Ok(Value::Number(x.max(y)))
}
