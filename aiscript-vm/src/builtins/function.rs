use crate::{Value, VmError, vm::State};

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
    let vec = match &args[0] {
        Value::List(list) => &list.borrow().data,
        _ => {
            return Err(VmError::RuntimeError(
                "map() first argument must be an array.".into(),
            ));
        }
    };

    // Get the function id
    let function = match args[1] {
        Value::Closure(ref f) => f.function,
        Value::NativeFunction(_) => {
            return Err(VmError::RuntimeError(
                "map() doesn't support native functions yet.".into(),
            ));
        }
        _ => {
            return Err(VmError::RuntimeError(
                "map() second argument must be a function.".into(),
            ));
        }
    };

    // Create result array
    let mut result = Vec::with_capacity(vec.len());

    // Apply function to each element
    for value in vec.iter() {
        // Prepare arguments for the function call
        let call_args = vec![*value];

        // Call the function and convert result
        result.push(state.eval_function(function, &call_args)?);
    }

    Ok(Value::array(state, result))
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
    let vec = match &args[0] {
        Value::List(list) => &list.borrow().data,
        _ => {
            return Err(VmError::RuntimeError(
                "filter() first argument must be an array.".into(),
            ));
        }
    };

    // Get the function id
    let function = match args[1] {
        Value::Closure(ref f) => f.function,
        Value::NativeFunction(_) => {
            return Err(VmError::RuntimeError(
                "filter() doesn't support native functions yet.".into(),
            ));
        }
        _ => {
            return Err(VmError::RuntimeError(
                "filter() second argument must be a function.".into(),
            ));
        }
    };

    // Create result array
    let mut result = Vec::with_capacity(vec.len());

    // Filter elements based on predicate function
    for value in vec.iter() {
        // Prepare arguments for the function call
        let call_args = vec![*value];

        // Call function and check if it returns true
        if state.eval_function(function, &call_args)?.is_true() {
            result.push(*value);
        }
    }

    Ok(Value::array(state, result))
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
            Value::List(list) => arrays.push(list),
            _ => {
                return Err(VmError::RuntimeError(
                    "zip() arguments must be arrays.".into(),
                ));
            }
        }
    }

    // Find the length of the shortest array
    let min_len = arrays
        .iter()
        .map(|arr| arr.borrow().data.len())
        .min()
        .unwrap_or(0);

    // Create result array
    let mut result = Vec::new();

    // Zip the arrays together
    for i in 0..min_len {
        let mut tuple = Vec::new();
        for arr in arrays.iter() {
            tuple.push(arr.borrow().data[i]);
        }
        result.push(Value::array(state, tuple));
    }

    Ok(Value::array(state, result))
}
