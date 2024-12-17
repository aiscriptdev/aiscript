use crate::{vm::State, Value, VmError};
use gc_arena::{Gc, RefLock};

pub(super) fn map<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 2 {
        return Err(VmError::RuntimeError(
            "map() takes exactly 2 arguments.".into(),
        ));
    }

    // Get the iterable
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => {
            return Err(VmError::RuntimeError(
                "map() first argument must be an array.".into(),
            ))
        }
    };

    // Get the function id
    let function = match args[1] {
        Value::Closure(ref f) => f.function,
        Value::NativeFunction(_) => {
            return Err(VmError::RuntimeError(
                "map() doesn't support native functions yet.".into(),
            ))
        }
        _ => {
            return Err(VmError::RuntimeError(
                "map() second argument must be a function.".into(),
            ))
        }
    };

    let arr = arr.borrow();
    // Create result array
    let mut result = Vec::with_capacity(arr.len());

    // Apply function to each element
    for value in arr.iter() {
        // Prepare arguments for the function call
        let call_args = vec![*value];

        // Call the function and convert result
        result.push(state.eval_function(function, &call_args)?);
    }

    Ok(Value::Array(Gc::new(state, RefLock::new(result))))
}

pub(super) fn filter<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() != 2 {
        return Err(VmError::RuntimeError(
            "filter() takes exactly 2 arguments.".into(),
        ));
    }

    // Get the iterable
    let arr = match &args[0] {
        Value::Array(arr) => arr,
        _ => {
            return Err(VmError::RuntimeError(
                "filter() first argument must be an array.".into(),
            ))
        }
    };

    // Get the function id
    let function = match args[1] {
        Value::Closure(ref f) => f.function,
        Value::NativeFunction(_) => {
            return Err(VmError::RuntimeError(
                "filter() doesn't support native functions yet.".into(),
            ))
        }
        _ => {
            return Err(VmError::RuntimeError(
                "filter() second argument must be a function.".into(),
            ))
        }
    };

    let arr = arr.borrow();
    // Create result array
    let mut result = Vec::with_capacity(arr.len());

    // Filter elements based on predicate function
    for value in arr.iter() {
        // Prepare arguments for the function call
        let call_args = vec![*value];

        // Call function and check if it returns true
        if state.eval_function(function, &call_args)?.is_true() {
            result.push(*value);
        }
    }

    Ok(Value::Array(Gc::new(state, RefLock::new(result))))
}

pub(super) fn zip<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.len() < 2 {
        return Err(VmError::RuntimeError(
            "zip() takes at least 2 arguments.".into(),
        ));
    }

    // Get all arrays and validate input
    let mut arrays = Vec::new();
    for arg in &args {
        match arg {
            Value::Array(arr) => arrays.push(arr),
            _ => {
                return Err(VmError::RuntimeError(
                    "zip() arguments must be arrays.".into(),
                ))
            }
        }
    }

    // Find the length of the shortest array
    let min_len = arrays
        .iter()
        .map(|arr| arr.borrow().len())
        .min()
        .unwrap_or(0);

    // Create result array
    let mut result = Vec::new();

    // Zip the arrays together
    for i in 0..min_len {
        let mut tuple = Vec::new();
        for arr in arrays.iter() {
            tuple.push(arr.borrow()[i]);
        }
        result.push(Value::Array(Gc::new(state, RefLock::new(tuple))));
    }

    Ok(Value::Array(Gc::new(state, RefLock::new(result))))
}
