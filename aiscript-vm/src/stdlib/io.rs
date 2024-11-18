use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::Path,
};

use gc_arena::{Gc, Mutation};

use crate::{
    module::ModuleKind,
    string_arg,
    value::Value,
    vm::{Context, VmError},
};

pub fn create_io_module(ctx: Context) -> ModuleKind {
    let name = ctx.intern(b"std.io");

    let exports = [
        // File reading/writing
        (
            ctx.intern(b"read_file"),
            Value::NativeFunction(io_read_file),
        ),
        // (
        //     ctx.intern(b"read_bytes"),
        //     Value::NativeFunction(io_read_bytes),
        // ),
        (
            ctx.intern(b"read_lines"),
            Value::NativeFunction(io_read_lines),
        ),
        (
            ctx.intern(b"write_file"),
            Value::NativeFunction(io_write_file),
        ),
        (
            ctx.intern(b"append_file"),
            Value::NativeFunction(io_append_file),
        ),
        // Standard IO
        (ctx.intern(b"print"), Value::NativeFunction(io_print)),
        (ctx.intern(b"println"), Value::NativeFunction(io_println)),
        (ctx.intern(b"input"), Value::NativeFunction(io_input)),
        // File/directory operations
        (ctx.intern(b"exists"), Value::NativeFunction(io_exists)),
        (ctx.intern(b"is_file"), Value::NativeFunction(io_is_file)),
        (ctx.intern(b"is_dir"), Value::NativeFunction(io_is_dir)),
        (
            ctx.intern(b"create_dir"),
            Value::NativeFunction(io_create_dir),
        ),
        (
            ctx.intern(b"remove_file"),
            Value::NativeFunction(io_remove_file),
        ),
        (
            ctx.intern(b"remove_dir"),
            Value::NativeFunction(io_remove_dir),
        ),
        (ctx.intern(b"rename"), Value::NativeFunction(io_rename)),
    ]
    .into_iter()
    .collect();
    ModuleKind::Native { name, exports }
}

// File reading functions
fn io_read_file<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "read_file")?.to_string();
    match fs::read_to_string(&path) {
        Ok(content) => Ok(Value::IoString(Gc::new(mc, content))),
        Err(e) => Err(VmError::RuntimeError(format!(
            "Failed to read file '{}': {}",
            path, e
        ))),
    }
}

// fn io_read_bytes<'gc>(
//     _mc: &'gc Mutation<'gc>,
//     args: Vec<Value<'gc>>,
// ) -> Result<Value<'gc>, VmError> {
//     let path = string_arg!(args, 0, "read_bytes")?.to_string();
//     match fs::read(&path) {
//         Ok(bytes) => {
//             // Convert bytes to a comma-separated string of numbers
//             let bytes_str = bytes
//                 .iter()
//                 .map(|b| b.to_string())
//                 .collect::<Vec<_>>()
//                 .join(",");
//             Ok(Value::String(
//                 args[0].context().intern(bytes_str.as_bytes()),
//             ))
//         }
//         Err(e) => Err(VmError::RuntimeError(format!(
//             "Failed to read bytes from '{}': {}",
//             path, e
//         ))),
//     }
// }

fn io_read_lines<'gc>(
    mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "read_lines")?.to_string();
    let file = File::open(&path)
        .map_err(|e| VmError::RuntimeError(format!("Failed to open file '{}': {}", path, e)))?;

    let lines: Vec<String> = BufReader::new(file)
        .lines()
        .collect::<Result<_, _>>()
        .map_err(|e| VmError::RuntimeError(format!("Failed to read lines: {}", e)))?;

    let content = lines.join("\n");
    Ok(Value::IoString(Gc::new(mc, content)))
}

// File writing functions
fn io_write_file<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "write_file")?.to_string();
    let content = string_arg!(args, 1, "write_file")?.to_string();

    fs::write(&path, content)
        .map_err(|e| VmError::RuntimeError(format!("Failed to write to file '{}': {}", path, e)))?;

    Ok(Value::Boolean(true))
}

fn io_append_file<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "append_file")?.to_string();
    let content = string_arg!(args, 1, "append_file")?.to_string();

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| VmError::RuntimeError(format!("Failed to open file '{}': {}", path, e)))?;

    file.write_all(content.as_bytes()).map_err(|e| {
        VmError::RuntimeError(format!("Failed to append to file '{}': {}", path, e))
    })?;

    Ok(Value::Boolean(true))
}

// Standard input/output functions
fn io_print<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let text = string_arg!(args, 0, "print")?.to_string();
    print!("{}", text);
    io::stdout()
        .flush()
        .map_err(|e| VmError::RuntimeError(format!("Failed to flush stdout: {}", e)))?;
    Ok(Value::Boolean(true))
}

fn io_println<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let text = string_arg!(args, 0, "println")?.to_string();
    println!("{}", text);
    Ok(Value::Boolean(true))
}

fn io_input<'gc>(mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    // Optional input
    if let Some(value) = args.first() {
        let input = value.as_string()?.to_string();
        print!("{}", input);
        io::stdout()
            .flush()
            .map_err(|e| VmError::RuntimeError(format!("Failed to flush stdout: {}", e)))?;
    }

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| VmError::RuntimeError(format!("Failed to read input: {}", e)))?;

    // Trim the trailing newline
    Ok(Value::IoString(Gc::new(mc, input.trim_end().to_owned())))
}

// File/directory operations
fn io_exists<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "exists")?.to_string();
    Ok(Value::Boolean(Path::new(&path).exists()))
}

fn io_is_file<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "is_file")?.to_string();
    Ok(Value::Boolean(Path::new(&path).is_file()))
}

fn io_is_dir<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "is_dir")?.to_string();
    Ok(Value::Boolean(Path::new(&path).is_dir()))
}

fn io_create_dir<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "create_dir")?.to_string();
    fs::create_dir_all(&path).map_err(|e| {
        VmError::RuntimeError(format!("Failed to create directory '{}': {}", path, e))
    })?;
    Ok(Value::Boolean(true))
}

fn io_remove_file<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "remove_file")?.to_string();
    fs::remove_file(&path)
        .map_err(|e| VmError::RuntimeError(format!("Failed to remove file '{}': {}", path, e)))?;
    Ok(Value::Boolean(true))
}

fn io_remove_dir<'gc>(
    _mc: &'gc Mutation<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    let path = string_arg!(args, 0, "remove_dir")?.to_string();
    let recursive = args.get(1).map(|v| v.as_boolean()).unwrap_or(false);

    if recursive {
        fs::remove_dir_all(&path)
    } else {
        fs::remove_dir(&path)
    }
    .map_err(|e| VmError::RuntimeError(format!("Failed to remove directory '{}': {}", path, e)))?;

    Ok(Value::Boolean(true))
}

fn io_rename<'gc>(_mc: &'gc Mutation<'gc>, args: Vec<Value<'gc>>) -> Result<Value<'gc>, VmError> {
    let from = string_arg!(args, 0, "rename")?.to_string();
    let to = string_arg!(args, 1, "rename")?.to_string();

    fs::rename(&from, &to).map_err(|e| {
        VmError::RuntimeError(format!("Failed to rename '{}' to '{}': {}", from, to, e))
    })?;

    Ok(Value::Boolean(true))
}
