use std::{
    fmt::{self, Write as _},
    str,
};

/// Display potentially non-UTF8 string in a lossy way, *without* allocating a new buffer to hold
/// the string.
pub fn display_utf8_lossy(input: &[u8]) -> impl fmt::Display + '_ {
    struct StringDisplay<'a>(&'a [u8]);

    impl<'a> fmt::Display for StringDisplay<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let mut input = self.0;
            loop {
                match str::from_utf8(input) {
                    Ok(valid) => {
                        f.write_str(valid)?;
                        break;
                    }
                    Err(error) => {
                        let (valid, after_valid) = input.split_at(error.valid_up_to());
                        f.write_str(str::from_utf8(valid).unwrap())?;
                        f.write_str("\u{FFFD}")?;

                        if let Some(invalid_sequence_length) = error.error_len() {
                            input = &after_valid[invalid_sequence_length..]
                        } else {
                            break;
                        }
                    }
                }
            }
            Ok(())
        }
    }

    StringDisplay(input)
}

/// Like [`display_utf8_lossy`], returns an `impl fmt::Debug` type that uses debug character escapes
/// and surrounds the string by '"' characters.
pub fn debug_utf8_lossy(input: &[u8]) -> impl fmt::Debug + '_ {
    struct StringDebug<'a>(&'a [u8]);

    impl<'a> fmt::Debug for StringDebug<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fn write_escape_str(f: &mut fmt::Formatter, s: &str) -> fmt::Result {
                for c in s.chars() {
                    for c in c.escape_debug() {
                        f.write_char(c)?;
                    }
                }
                Ok(())
            }

            fn write_escape_bytes(f: &mut fmt::Formatter, s: &[u8]) -> fmt::Result {
                fn to_hex_digit(b: u8) -> Option<char> {
                    if b < 10 {
                        char::from_u32('0' as u32 + b as u32)
                    } else if b < 16 {
                        char::from_u32('a' as u32 + (b as u32 - 10))
                    } else {
                        None
                    }
                }

                for b in s {
                    f.write_str("\\x")?;
                    f.write_char(to_hex_digit(b >> 4).unwrap())?;
                    f.write_char(to_hex_digit(b & 0xf).unwrap())?;
                }
                Ok(())
            }

            f.write_char('"')?;
            let mut input = self.0;
            loop {
                match str::from_utf8(input) {
                    Ok(valid) => {
                        write_escape_str(f, valid)?;
                        break;
                    }
                    Err(error) => {
                        let (valid, after_valid) = input.split_at(error.valid_up_to());
                        write_escape_str(f, str::from_utf8(valid).unwrap())?;

                        if let Some(invalid_sequence_length) = error.error_len() {
                            write_escape_bytes(f, &after_valid[0..invalid_sequence_length])?;
                            input = &after_valid[invalid_sequence_length..];
                        } else {
                            write_escape_bytes(f, &after_valid[0..])?;
                            break;
                        }
                    }
                }
            }
            f.write_char('"')?;

            Ok(())
        }
    }

    StringDebug(input)
}
