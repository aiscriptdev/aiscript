use crate::{Value, VmError, vm::State};
use std::fmt::Write;

/// Print objects to the text stream file, separated by sep and followed by end.
///
/// Args:
/// *objects: Objects to print.
/// sep (str): String inserted between values. Default: ' '
/// end (str): String appended after the last value. Default: '\n'
/// file: A file-like object (stream). Defaults to the current stdout
/// flush (bool): Whether to forcibly flush the stream. Default: False
///
/// fn print(*objects, sep=" ", end="\n", file=nil, flush=false) {}
pub(super) fn print<'gc>(
    _state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    // Extract keyword arguments with defaults
    let mut sep = " ";
    let mut end = "\n";
    // let mut file = None;
    let mut flush = false;

    // Build arguments iterator to handle both positional and keyword args
    let mut i = 0;
    let mut positional = Vec::new();

    while i < args.len() {
        match args[i] {
            Value::String(key) if i + 1 < args.len() => {
                match key.to_str().unwrap() {
                    "sep" => {
                        sep = args[i + 1].as_string()?.to_str().unwrap();
                        i += 2;
                    }
                    "end" => {
                        end = args[i + 1].as_string()?.to_str().unwrap();
                        i += 2;
                    }
                    "file" => {
                        // For now, just ignore file argument since we only support stdout
                        i += 2;
                    }
                    "flush" => {
                        flush = args[i + 1].as_boolean();
                        i += 2;
                    }
                    _ => {
                        // Not a keyword arg, treat as positional
                        positional.push(&args[i]);
                        i += 1;
                    }
                }
            }
            _ => {
                positional.push(&args[i]);
                i += 1;
            }
        }
    }

    // Build the output string
    let mut output = String::new();

    for (i, arg) in positional.iter().enumerate() {
        if i > 0 {
            output.push_str(sep);
        }
        write!(output, "{}", arg).unwrap();
    }
    output.push_str(end);

    print!("{}", output);
    if flush {
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
    }

    Ok(Value::Nil)
}
