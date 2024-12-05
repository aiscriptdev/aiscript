use crate::{Token, TokenType};

#[derive(Default)]
pub struct ErrorReporter {
    pub panic_mode: bool,
    pub had_error: bool,
}

impl ErrorReporter {
    pub fn new() -> Self {
        Self {
            panic_mode: false,
            had_error: false,
        }
    }

    pub fn error_at(&mut self, token: Token<'_>, message: &str) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        eprint!("[line {}] Error", token.line);
        if token.kind == TokenType::Eof {
            eprint!(" at end");
        } else if token.kind == TokenType::Invalid {
            // Do nothing.
        } else {
            eprint!(" at '{}'", token.lexeme);
        }
        eprintln!(": {message}");
        self.had_error = true;
    }
}
