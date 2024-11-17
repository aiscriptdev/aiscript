use std::{iter::Peekable, str::CharIndices};

mod character_tests;
mod tests;

/// Represents different types of tokens in the language
#[derive(Debug, Hash, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    // Delimiters
    OpenParen,    // (
    CloseParen,   // )
    OpenBrace,    // {
    CloseBrace,   // }
    OpenBracket,  // [
    CloseBracket, // ]

    // Punctuation
    Comma,     // ,
    Dot,       // .
    Minus,     // -
    Plus,      // +
    Semicolon, // ;
    Slash,     // /
    Star,      // *
    Colon,     // :
    Percent,   // %

    // Comparison and logical operators
    Bang,         // !
    BangEqual,    // !=
    Equal,        // =
    EqualEqual,   // ==
    Greater,      // >
    GreaterEqual, // >=
    Less,         // <
    LessEqual,    // <=
    Arrow,        // ->
    StarStar,     // **

    // Compound assignment
    PlusEqual,    // +=
    MinusEqual,   // -=
    StarEqual,    // *=
    SlashEqual,   // /=
    PercentEqual, // %=

    // Literals
    Identifier, // Variable/function names
    String,     // "string literal"
    Number,     // 123, 123.45
    Doc,        // """docstring"""

    // Keywords
    And,
    Break,
    Class,
    Const,
    Continue,
    Else,
    False,
    For,
    Fn,
    If,
    Nil,
    Or,
    Print,
    Pub,
    Return,
    Super,
    This,
    True,
    Let,
    Use,
    While,

    // AI-specific keywords
    AI,
    Prompt,
    Agent,

    // Special tokens
    Error, // Lexing error
    Eof,   // End of file
}

/// Represents a single token in the source code
#[derive(Debug, Hash, Copy, Clone, Eq, PartialEq)]
pub struct Token<'a> {
    /// The actual text of the token
    pub lexeme: &'a str,
    /// The line number where the token appears
    pub line: u32,
    /// The type of the token
    pub kind: TokenType,
}

impl Default for Token<'_> {
    fn default() -> Self {
        Self {
            kind: TokenType::Eof,
            lexeme: "",
            line: 1,
        }
    }
}

impl<'a> Token<'a> {
    /// Creates a new token with the given type, text, and line number
    pub fn new(kind: TokenType, origin: &'a str, line: u32) -> Self {
        Token {
            kind,
            lexeme: origin,
            line,
        }
    }

    /// Creates a new identifier token (available with v1 feature)
    #[cfg(feature = "v1")]
    pub fn identifier(name: &'a str) -> Self {
        Token::new(TokenType::Identifier, name, 0)
    }
}

/// Scanner/Lexer for tokenizing source code
pub struct Scanner<'a> {
    /// The complete source code being scanned
    pub source: &'a str,
    /// Character iterator for the source
    iter: Peekable<CharIndices<'a>>,
    /// Start position of current token (in bytes)
    pub start: usize,
    /// Current position in the source (in bytes)
    pub current: usize,
    /// Current line number
    pub line: u32,
    /// Whether we've reached the end of file
    is_eof: bool,
}

impl<'a> Scanner<'a> {
    /// Creates a new Scanner for the given source code
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            iter: source.char_indices().peekable(),
            start: 0,
            current: 0,
            line: 1,
            is_eof: false,
        }
    }

    /// Advances the scanner one character forward
    fn advance(&mut self) -> Option<char> {
        match self.iter.next() {
            Some((pos, ch)) => {
                self.current = pos + ch.len_utf8();
                Some(ch)
            }
            None => None,
        }
    }

    /// Peeks at the next character without consuming it
    fn peek(&mut self) -> Option<char> {
        self.iter.peek().map(|&(_, c)| c)
    }

    /// Looks ahead at the next N characters without consuming them
    fn check_next(&self, n: usize) -> Option<&str> {
        let mut chars = self.source[self.current..].char_indices();
        match chars.nth(n - 1) {
            Some((end_offset, ch)) => {
                let end = self.current + end_offset + ch.len_utf8();
                if end <= self.source.len() {
                    Some(&self.source[self.current..end])
                } else {
                    None
                }
            }
            None => None,
        }
    }

    /// Returns the next 2 characters as a string slice
    fn next2(&mut self) -> &str {
        let mut chars = self.source[self.current..].char_indices();
        match chars.nth(1) {
            Some((end_offset, ch)) => {
                let end = self.current + end_offset + ch.len_utf8();
                &self.source[self.current..end]
            }
            None => &self.source[self.current..],
        }
    }

    /// Skips whitespace and comments
    fn skip_white_spaces(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\r' | '\t' => {
                    self.advance();
                }
                '\n' => {
                    self.line += 1;
                    self.advance();
                }
                '/' => {
                    if self.next2() == "//" {
                        while matches!(self.peek(), Some(c) if c != '\n') {
                            self.advance();
                        }
                    } else {
                        return;
                    }
                }
                _ => return,
            }
        }
    }

    /// Creates a token of the given type using the current lexeme
    fn make_token(&self, kind: TokenType) -> Token<'a> {
        Token {
            kind,
            lexeme: &self.source[self.start..self.current],
            line: self.line,
        }
    }

    fn scan_docstring(&mut self) -> Token<'a> {
        // Skip the opening quotes
        for _ in 0..3 {
            self.advance();
        }
        // Don't set self.start = self.current, since we want
        // to include the first character of the docstring
        self.start += 3;

        loop {
            match self.peek() {
                Some(c) => {
                    if c == '"' && self.next2() == "\"\"" {
                        break;
                    }
                    // Track line numbers
                    if c == '\n' {
                        self.line += 1;
                    }
                    self.advance();
                }
                // Check for EOF
                None => {
                    return Token::new(TokenType::Error, "Unterminated docstring.", self.line);
                }
            }
        }

        let token = if self.start == self.current - 1 {
            // Empty docstring
            Token {
                kind: TokenType::Doc,
                lexeme: "",
                line: self.line,
            }
        } else {
            let mut token = self.make_token(TokenType::Doc);
            token.lexeme = token.lexeme.trim();
            token
        };

        while let Some(c) = self.peek() {
            if c == '"' {
                self.advance();
            } else {
                break;
            }
        }
        token
    }

    /// Scans numeric literals (integers and floats)
    fn scan_number(&mut self) -> Token<'a> {
        self.consume_digits();
        // Look for a fractional part.
        if self.source[self.current..].len() >= 2 {
            let mut next_two_chars = self.source[self.current..self.current + 2].chars();
            let (maybe_dot, maybe_digit) = (next_two_chars.next(), next_two_chars.next());
            if maybe_dot == Some('.') && matches!(maybe_digit, Some(c) if c.is_ascii_digit()) {
                // Consume the "."
                self.advance();

                self.consume_digits();
            }
        }

        self.make_token(TokenType::Number)
    }

    /// Helper method to consume consecutive digits
    fn consume_digits(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.advance();
        }
    }

    /// Scans identifiers and keywords
    fn scan_identifier(&mut self) -> Token<'a> {
        while matches!(self.peek(), Some(c) if c.is_alphanumeric() || c == '_') {
            self.advance();
        }

        let text = &self.source[self.start..self.current];
        let kind = match text {
            "ai" => TokenType::AI,
            "agent" => TokenType::Agent,
            "and" => TokenType::And,
            "break" => TokenType::Break,
            "class" => TokenType::Class,
            "const" => TokenType::Const,
            "continue" => TokenType::Continue,
            "else" => TokenType::Else,
            "false" => TokenType::False,
            "for" => TokenType::For,
            "fn" => TokenType::Fn,
            "if" => TokenType::If,
            "nil" => TokenType::Nil,
            "or" => TokenType::Or,
            "print" => TokenType::Print,
            "prompt" => TokenType::Prompt,
            "pub" => TokenType::Pub,
            "return" => TokenType::Return,
            "super" => TokenType::Super,
            "this" => TokenType::This,
            "true" => TokenType::True,
            "let" => TokenType::Let,
            "use" => TokenType::Use,
            "while" => TokenType::While,
            _ => TokenType::Identifier,
        };

        self.make_token(kind)
    }

    /// Scans the next token from the source
    fn scan_token(&mut self) -> Token<'a> {
        self.skip_white_spaces();

        self.start = self.current;

        let c = match self.advance() {
            Some(c) => c,
            None => return Token::new(TokenType::Eof, "", self.line),
        };

        match c {
            '(' => self.make_token(TokenType::OpenParen),
            ')' => self.make_token(TokenType::CloseParen),
            '[' => self.make_token(TokenType::OpenBracket),
            ']' => self.make_token(TokenType::CloseBracket),
            '{' => self.make_token(TokenType::OpenBrace),
            '}' => self.make_token(TokenType::CloseBrace),
            ';' => self.make_token(TokenType::Semicolon),
            ',' => self.make_token(TokenType::Comma),
            '.' => self.make_token(TokenType::Dot),
            '-' => {
                let p = self.peek();
                let kind = if p == Some('>') {
                    self.advance();
                    TokenType::Arrow
                } else if p == Some('=') {
                    self.advance();
                    TokenType::MinusEqual
                } else {
                    TokenType::Minus
                };
                self.make_token(kind)
            }
            '+' => {
                let kind = if self.peek() == Some('=') {
                    self.advance();
                    TokenType::PlusEqual
                } else {
                    TokenType::Plus
                };
                self.make_token(kind)
            }
            '/' => {
                let kind = if self.peek() == Some('=') {
                    self.advance();
                    TokenType::SlashEqual
                } else {
                    TokenType::Slash
                };
                self.make_token(kind)
            }
            '*' => {
                let p = self.peek();
                let kind = if p == Some('*') {
                    self.advance();
                    TokenType::StarStar
                } else if p == Some('=') {
                    self.advance();
                    TokenType::StarEqual
                } else {
                    TokenType::Star
                };
                self.make_token(kind)
            }
            ':' => self.make_token(TokenType::Colon),
            '%' => {
                let kind = if self.peek() == Some('=') {
                    self.advance();
                    TokenType::PercentEqual
                } else {
                    TokenType::Percent
                };
                self.make_token(kind)
            }
            '!' => {
                let kind = if self.peek() == Some('=') {
                    self.advance();
                    TokenType::BangEqual
                } else {
                    TokenType::Bang
                };
                self.make_token(kind)
            }
            '=' => {
                let kind = if self.peek() == Some('=') {
                    self.advance();
                    TokenType::EqualEqual
                } else {
                    TokenType::Equal
                };
                self.make_token(kind)
            }
            '<' => {
                let kind = if self.peek() == Some('=') {
                    self.advance();
                    TokenType::LessEqual
                } else {
                    TokenType::Less
                };
                self.make_token(kind)
            }
            '>' => {
                let kind = if self.peek() == Some('=') {
                    self.advance();
                    TokenType::GreaterEqual
                } else {
                    TokenType::Greater
                };
                self.make_token(kind)
            }
            '"' => {
                // Check for docstring
                if let Some("\"\"") = self.check_next(2) {
                    self.scan_docstring()
                } else {
                    // Regular string
                    while let Some(ch) = self.peek() {
                        match ch {
                            '"' => break,
                            '\n' => {
                                self.line += 1;
                                self.advance();
                            }
                            _ => {
                                self.advance();
                            }
                        }
                    }

                    match self.peek() {
                        Some('"') => {
                            self.advance();
                            self.make_token(TokenType::String)
                        }
                        _ => Token::new(TokenType::Error, "Unterminated string.", self.line),
                    }
                }
            }

            c if c.is_ascii_digit() => self.scan_number(),
            c if c.is_alphabetic() || c == '_' => self.scan_identifier(),
            _ => Token::new(TokenType::Error, "Unexpected character.", self.line),
        }
    }
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_eof {
            None
        } else {
            let token = self.scan_token();
            self.is_eof = token.kind == TokenType::Eof;
            Some(token)
        }
    }
}
