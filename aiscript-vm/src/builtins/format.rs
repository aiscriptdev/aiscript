use crate::{Value, VmError, vm::State};
use aiscript_arena::Gc;
use std::fmt::Write;

/// Format function that implements Python-like string formatting
pub(super) fn format<'gc>(
    state: &mut State<'gc>,
    args: Vec<Value<'gc>>,
) -> Result<Value<'gc>, VmError> {
    if args.is_empty() {
        return Err(VmError::RuntimeError(
            "format() missing required argument.".into(),
        ));
    }

    let template = args[0].as_string()?.to_str().unwrap();
    let format_args = &args[1..];

    let result = format_string(template, format_args)?;
    Ok(Value::IoString(Gc::new(state, result)))
}

#[derive(Debug, Default)]
struct FormatSpec {
    fill: Option<char>,
    align: Option<char>,
    sign: Option<char>,
    width: Option<usize>,
    precision: Option<usize>,
    format_type: Option<char>,
}

impl FormatSpec {
    fn parse(spec: &str) -> Result<Self, VmError> {
        let mut result = FormatSpec::default();
        let mut chars = spec.chars().peekable();

        // Skip the initial ':'
        if chars.next_if_eq(&':').is_some() {
            // Parse fill and align
            let mut next = chars.peek().copied();
            if let Some(c) = next {
                if chars.clone().nth(1).is_some_and(|n| "<>^".contains(n)) {
                    result.fill = Some(c);
                    chars.next();
                    next = chars.peek().copied();
                }
            }

            // Parse align
            if let Some(c) = next {
                if "<>^".contains(c) {
                    result.align = Some(c);
                    chars.next();
                }
            }

            // Parse sign
            if let Some(c) = chars.peek() {
                if "+-".contains(*c) {
                    result.sign = Some(*c);
                    chars.next();
                }
            }

            // Parse width
            let mut width = String::new();
            while let Some(c) = chars.peek() {
                if c.is_ascii_digit() {
                    width.push(*c);
                    chars.next();
                } else {
                    break;
                }
            }
            if !width.is_empty() {
                result.width = Some(width.parse().unwrap());
            }

            // Parse precision
            if chars.next_if_eq(&'.').is_some() {
                let mut precision = String::new();
                while let Some(c) = chars.peek() {
                    if c.is_ascii_digit() {
                        precision.push(*c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !precision.is_empty() {
                    result.precision = Some(precision.parse().unwrap());
                }
            }

            // Parse type
            if let Some(c) = chars.next() {
                if "dfsxXob".contains(c) {
                    result.format_type = Some(c);
                }
            }
        }

        Ok(result)
    }

    fn format(&self, value: &Value) -> Result<String, VmError> {
        let mut formatted = match value {
            Value::Number(n) => match self.format_type {
                Some('d') => format!("{:.0}", n),
                Some('x') => {
                    let int_val = *n as i64;
                    format!("{:x}", int_val)
                }
                Some('X') => {
                    let int_val = *n as i64;
                    format!("{:X}", int_val)
                }
                Some('o') => {
                    let int_val = *n as i64;
                    format!("{:o}", int_val)
                }
                Some('b') => {
                    let int_val = *n as i64;
                    format!("{:b}", int_val)
                }
                Some('f') => {
                    if let Some(precision) = self.precision {
                        format!("{:.*}", precision, n)
                    } else {
                        format!("{}", n)
                    }
                }
                _ => {
                    if let Some(precision) = self.precision {
                        format!("{:.*}", precision, n)
                    } else {
                        format!("{}", n)
                    }
                }
            },
            Value::String(s) => {
                let s = s.to_str().unwrap();
                if let Some(precision) = self.precision {
                    s[..s.chars().take(precision).map(|c| c.len_utf8()).sum()].to_string()
                } else {
                    s.to_string()
                }
            }
            _ => format!("{}", value),
        };

        // Apply width and alignment
        if let Some(width) = self.width {
            if formatted.chars().count() < width {
                let padding = width - formatted.chars().count();
                let fill_char = self.fill.unwrap_or(' ');

                match self.align.unwrap_or('>') {
                    '<' => {
                        formatted.extend(std::iter::repeat_n(fill_char, padding));
                    }
                    '>' => {
                        let mut new_string = String::new();
                        new_string.extend(std::iter::repeat_n(fill_char, padding));
                        new_string.push_str(&formatted);
                        formatted = new_string;
                    }
                    '^' => {
                        let left_pad = padding / 2;
                        let right_pad = padding - left_pad;
                        let mut new_string = String::new();
                        new_string.extend(std::iter::repeat_n(fill_char, left_pad));
                        new_string.push_str(&formatted);
                        new_string.extend(std::iter::repeat_n(fill_char, right_pad));
                        formatted = new_string;
                    }
                    _ => unreachable!(),
                }
            }
        }

        Ok(formatted)
    }
}

fn format_string(template: &str, args: &[Value]) -> Result<String, VmError> {
    let mut result = String::new();
    let mut chars = template.chars().peekable();
    let mut arg_index = 0;

    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.next_if_eq(&'{').is_some() {
                result.push('{');
                continue;
            }

            // Collect format spec
            let mut spec = String::new();
            let mut nested = 1;

            for c in chars.by_ref() {
                match c {
                    '{' => nested += 1,
                    '}' => {
                        nested -= 1;
                        if nested == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                spec.push(c);
            }

            if nested != 0 {
                return Err(VmError::RuntimeError(
                    "Unmatched brace in format string.".into(),
                ));
            }

            let format_spec = FormatSpec::parse(&spec)?;

            if arg_index >= args.len() {
                return Err(VmError::RuntimeError(
                    "Not enough arguments for format string.".into(),
                ));
            }

            let formatted = format_spec.format(&args[arg_index])?;
            write!(result, "{}", formatted).map_err(|e| VmError::RuntimeError(e.to_string()))?;

            arg_index += 1;
        } else if c == '}' {
            if chars.next_if_eq(&'}').is_some() {
                result.push('}');
            } else {
                return Err(VmError::RuntimeError(
                    "Single '}' encountered in format string.".into(),
                ));
            }
        } else {
            result.push(c);
        }
    }

    Ok(result)
}
