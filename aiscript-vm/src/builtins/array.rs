use aiscript_arena::Mutation;
use std::collections::HashMap;

use crate::string::InternedString;
use crate::{BuiltinMethod, Value, VmError, float_arg, vm::Context};

pub(crate) fn define_array_methods(ctx: Context) -> HashMap<InternedString, BuiltinMethod> {
    [
        // Basic operations
        ("append", BuiltinMethod(append)),
        ("extend", BuiltinMethod(extend)),
        ("insert", BuiltinMethod(insert)),
        ("remove", BuiltinMethod(remove)),
        ("pop", BuiltinMethod(pop)),
        ("clear", BuiltinMethod(clear)),
        // Search operations
        ("index", BuiltinMethod(index)),
        ("count", BuiltinMethod(count)),
        // Ordering operations
        ("sort", BuiltinMethod(sort)),
        ("reverse", BuiltinMethod(reverse)),
        // Subarray operations
        ("slice", BuiltinMethod(slice)),
    ]
    .into_iter()
    .map(|(name, f)| (ctx.intern_static(name), f))
    .collect()
}

// Add an item to the end of the list
fn append<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    if args.is_empty() {
        return Err(VmError::RuntimeError("append: expected 1 argument".into()));
    }

    let value = args[0];
    list.borrow_mut(_mc).push(value);

    // Return the modified list for method chaining
    Ok(receiver)
}

// Extend the list by appending all items from another list
fn extend<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "extend: expected 1 array argument".into(),
        ));
    }

    match &args[0] {
        Value::List(other_list) => {
            let items = &other_list.borrow().data;
            let mut list_mut = list.borrow_mut(mc);
            for item in items {
                list_mut.push(*item);
            }
            Ok(receiver)
        }
        _ => Err(VmError::RuntimeError(
            "extend: argument must be an array".into(),
        )),
    }
}

// Insert an item at a given position
fn insert<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    if args.len() < 2 {
        return Err(VmError::RuntimeError(
            "insert: expected 2 arguments (index, value)".into(),
        ));
    }

    let index = float_arg!(&args, 0, "insert")? as usize;
    let value = args[1];

    let mut list_mut = list.borrow_mut(mc);

    // Check if index is valid
    if index > list_mut.data.len() {
        return Err(VmError::RuntimeError(format!(
            "insert: index {} out of range",
            index
        )));
    }

    // Insert the value at the specified position
    list_mut.data.insert(index, value);

    Ok(receiver)
}

// Remove the first item from the list whose value is equal to x
fn remove<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    if args.is_empty() {
        return Err(VmError::RuntimeError("remove: expected 1 argument".into()));
    }

    let value_to_remove = &args[0];

    let mut list_mut = list.borrow_mut(mc);
    if let Some(index) = list_mut
        .data
        .iter()
        .position(|item| item.equals(value_to_remove))
    {
        list_mut.data.remove(index);
        Ok(receiver)
    } else {
        Err(VmError::RuntimeError(format!(
            "remove: value {} not found in list",
            value_to_remove
        )))
    }
}

// Remove the item at the given position and return it
fn pop<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;
    let mut list_mut = list.borrow_mut(mc);

    // If list is empty, return an error
    if list_mut.data.is_empty() {
        return Err(VmError::RuntimeError(
            "pop: cannot pop from empty list".into(),
        ));
    }

    let index = if args.is_empty() {
        // Default to the last element if no index is provided
        list_mut.data.len() - 1
    } else {
        float_arg!(&args, 0, "pop")? as usize
    };

    // Check if index is valid
    if index >= list_mut.data.len() {
        return Err(VmError::RuntimeError(format!(
            "pop: index {} out of range",
            index
        )));
    }

    // Remove and return the value at the specified position
    Ok(list_mut.data.remove(index))
}

// Remove all items from the list
fn clear<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    let mut list_mut = list.borrow_mut(mc);
    list_mut.data.clear();

    Ok(receiver)
}

// Return zero-based index of the first item with value equal to x
fn index<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "index: expected at least 1 argument".into(),
        ));
    }

    let value_to_find = &args[0];

    // Get optional start and end parameters
    let start: usize = if args.len() > 1 {
        float_arg!(&args, 1, "index")? as usize
    } else {
        0
    };

    let end: usize = if args.len() > 2 {
        float_arg!(&args, 2, "index")? as usize
    } else {
        list.borrow().data.len()
    };

    // Validate start and end
    let list_len = list.borrow().data.len();
    let start = start.min(list_len);
    let end = end.min(list_len);

    // Search for the value in the specified range
    for (i, item) in list.borrow().data[start..end].iter().enumerate() {
        if item.equals(value_to_find) {
            // Return relative to original list
            return Ok(Value::Number((i + start) as f64));
        }
    }

    Err(VmError::RuntimeError(format!(
        "index: value {} not found in list",
        value_to_find
    )))
}

// Returns a shallow copy of a portion of the array
fn slice<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "slice: expected at least 1 argument".into(),
        ));
    }

    let start = float_arg!(&args, 0, "slice")? as isize;

    let list_ref = list.borrow();
    let list_len = list_ref.data.len() as isize;

    // Calculate start index (handle negative indices)
    let start_idx = if start < 0 {
        (list_len + start).max(0) as usize
    } else {
        start.min(list_len) as usize
    };

    // Calculate end index (handle negative indices and optional end parameter)
    let end_idx = if args.len() > 1 {
        let end = float_arg!(&args, 1, "slice")? as isize;
        if end < 0 {
            (list_len + end).max(0) as usize
        } else {
            end.min(list_len) as usize
        }
    } else {
        list_len as usize
    };

    // Create a new array with the sliced elements
    let start_idx = start_idx.min(end_idx); // Ensure start <= end
    let result = list_ref.data[start_idx..end_idx].to_vec();

    Ok(Value::array(mc, result))
}

// Return the number of times x appears in the list
fn count<'gc>(
    _mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    if args.is_empty() {
        return Err(VmError::RuntimeError("count: expected 1 argument".into()));
    }

    let value_to_count = &args[0];

    let count = list
        .borrow()
        .data
        .iter()
        .filter(|item| item.equals(value_to_count))
        .count();

    Ok(Value::Number(count as f64))
}

// Sort the items of the list in place
fn sort<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    // Check for optional reverse parameter
    let reverse = if !args.is_empty() {
        args[0].as_boolean()
    } else {
        false
    };

    let mut list_mut = list.borrow_mut(mc);

    // Currently only supporting numeric sorts
    // This could be expanded to support custom comparators
    if reverse {
        list_mut.data.sort_by(|a, b| {
            if let (Value::Number(x), Value::Number(y)) = (a, b) {
                y.partial_cmp(x).unwrap()
            } else {
                // For non-numeric values, just keep their order
                std::cmp::Ordering::Equal
            }
        });
    } else {
        list_mut.data.sort_by(|a, b| {
            if let (Value::Number(x), Value::Number(y)) = (a, b) {
                x.partial_cmp(y).unwrap()
            } else {
                // For non-numeric values, just keep their order
                std::cmp::Ordering::Equal
            }
        });
    }

    Ok(receiver)
}

// Reverse the elements of the list in place
fn reverse<'gc>(
    mc: &'gc Mutation<'gc>,
    receiver: Value<'gc>,
    _args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let list = receiver.as_array()?;

    let mut list_mut = list.borrow_mut(mc);
    list_mut.data.reverse();

    Ok(receiver)
}
