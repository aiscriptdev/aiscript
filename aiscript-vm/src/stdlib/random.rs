use rand::distributions::{Distribution, Uniform};
use rand::prelude::*;
use std::time::SystemTime;

use crate::{
    float_arg,
    module::ModuleKind,
    vm::{Context, State},
    NativeFn, Value, VmError,
};

pub fn create_random_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.random");

    let exports = [
        // Core random functions
        ("seed", Value::NativeFunction(NativeFn(random_seed))),
        ("random", Value::NativeFunction(NativeFn(random_float))),
        ("randint", Value::NativeFunction(NativeFn(random_int))),
        ("uniform", Value::NativeFunction(NativeFn(random_uniform))),
        // Range functions
        ("range", Value::NativeFunction(NativeFn(random_range))),
        ("choice", Value::NativeFunction(NativeFn(random_choice))),
        // Distribution functions
        ("normal", Value::NativeFunction(NativeFn(random_normal))),
        (
            "exponential",
            Value::NativeFunction(NativeFn(random_exponential)),
        ),
        // Boolean function
        ("bool", Value::NativeFunction(NativeFn(random_bool))),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect();

    ModuleKind::Native { name, exports }
}

thread_local! {
    static RNG: std::cell::RefCell<StdRng> = std::cell::RefCell::new(
        StdRng::from_entropy()
    );
}

fn random_seed<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let seed = if args.is_empty() {
        // Use current time as seed if none provided
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    } else {
        float_arg!(&args, 0, "seed")? as u64
    };

    RNG.with(|rng| {
        *rng.borrow_mut() = StdRng::seed_from_u64(seed);
    });

    Ok(Value::Number(seed as f64))
}

fn random_float<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if !args.is_empty() {
        return Err(VmError::RuntimeError("random() takes no arguments".into()));
    }

    RNG.with(|rng| {
        let val: f64 = rng.borrow_mut().gen();
        Ok(Value::Number(val))
    })
}

fn random_int<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    if args.len() != 2 {
        return Err(VmError::RuntimeError(
            "randint() takes exactly 2 arguments: min and max".into(),
        ));
    }

    let min = float_arg!(&args, 0, "randint")? as i64;
    let max = float_arg!(&args, 1, "randint")? as i64;

    if min > max {
        return Err(VmError::RuntimeError(
            "randint(): max must be greater than min".into(),
        ));
    }

    RNG.with(|rng| {
        let dist = Uniform::new_inclusive(min, max);
        let val = dist.sample(&mut *rng.borrow_mut());
        Ok(Value::Number(val as f64))
    })
}

fn random_uniform<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 2 {
        return Err(VmError::RuntimeError(
            "uniform() takes exactly 2 arguments: min and max".into(),
        ));
    }

    let min = float_arg!(&args, 0, "uniform")?;
    let max = float_arg!(&args, 1, "uniform")?;

    if min > max {
        return Err(VmError::RuntimeError(
            "uniform(): max must be greater than min".into(),
        ));
    }

    RNG.with(|rng| {
        let dist = Uniform::new(min, max);
        let val = dist.sample(&mut *rng.borrow_mut());
        Ok(Value::Number(val))
    })
}

fn random_range<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 3 {
        return Err(VmError::RuntimeError(
            "range() takes exactly 3 arguments: start, stop, and step".into(),
        ));
    }

    let start = float_arg!(&args, 0, "range")?;
    let stop = float_arg!(&args, 1, "range")?;
    let step = float_arg!(&args, 2, "range")?;

    if step == 0.0 {
        return Err(VmError::RuntimeError("range(): step cannot be zero".into()));
    }

    let num_steps = ((stop - start) / step).abs().floor() as i64;
    if num_steps <= 0 {
        return Err(VmError::RuntimeError(
            "range(): invalid range parameters".into(),
        ));
    }

    RNG.with(|rng| {
        let step_idx = rng.borrow_mut().gen_range(0..=num_steps);
        let val = start + (step_idx as f64 * step);
        Ok(Value::Number(val))
    })
}

fn random_choice<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 1 {
        return Err(VmError::RuntimeError(
            "choice() takes exactly 1 argument: array".into(),
        ));
    }

    match &args[0] {
        Value::Array(arr) => {
            let arr = arr.borrow();
            if arr.is_empty() {
                return Err(VmError::RuntimeError("choice(): array is empty".into()));
            }

            RNG.with(|rng| {
                let idx = rng.borrow_mut().gen_range(0..arr.len());
                Ok(arr[idx])
            })
        }
        _ => Err(VmError::RuntimeError(
            "choice(): argument must be an array".into(),
        )),
    }
}

fn random_normal<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let (mu, sigma) = match args.len() {
        0 => (0.0, 1.0), // Standard normal distribution
        2 => {
            let mu = float_arg!(&args, 0, "normal")?;
            let sigma = float_arg!(&args, 1, "normal")?;
            if sigma <= 0.0 {
                return Err(VmError::RuntimeError(
                    "normal(): standard deviation must be positive".into(),
                ));
            }
            (mu, sigma)
        }
        _ => {
            return Err(VmError::RuntimeError(
                "normal() takes either 0 or 2 arguments".into(),
            ))
        }
    };

    RNG.with(|rng| {
        // Box-Muller transform
        let u1: f64 = rng.borrow_mut().gen();
        let u2: f64 = rng.borrow_mut().gen();

        let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
        let val = mu + sigma * z;

        Ok(Value::Number(val))
    })
}

fn random_exponential<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let lambda = if args.is_empty() {
        1.0 // Default rate parameter
    } else if args.len() == 1 {
        let lambda = float_arg!(&args, 0, "exponential")?;
        if lambda <= 0.0 {
            return Err(VmError::RuntimeError(
                "exponential(): rate parameter must be positive".into(),
            ));
        }
        lambda
    } else {
        return Err(VmError::RuntimeError(
            "exponential() takes either 0 or 1 argument".into(),
        ));
    };

    RNG.with(|rng| {
        let u: f64 = rng.borrow_mut().gen();
        let val = -u.ln() / lambda;
        Ok(Value::Number(val))
    })
}

fn random_bool<'gc>(_state: &mut State<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let p = if args.is_empty() {
        0.5 // Default probability
    } else if args.len() == 1 {
        let p = float_arg!(&args, 0, "bool")?;
        if !(0.0..=1.0).contains(&p) {
            return Err(VmError::RuntimeError(
                "bool(): probability must be between 0 and 1".into(),
            ));
        }
        p
    } else {
        return Err(VmError::RuntimeError(
            "bool() takes either 0 or 1 argument".into(),
        ));
    };

    RNG.with(|rng| {
        let val: f64 = rng.borrow_mut().gen();
        Ok(Value::Boolean(val < p))
    })
}
