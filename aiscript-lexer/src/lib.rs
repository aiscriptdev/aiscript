use std::{
    iter::Peekable,
    mem,
    ops::{Deref, DerefMut},
    str::CharIndices,
};

pub use error_reporter::ErrorReporter;

mod character_tests;
mod error_reporter;
mod peakable;
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
    Comma,      // ,
    Dot,        // .
    Minus,      // -
    Plus,       // +
    Semicolon,  // ;
    Slash,      // /
    Star,       // *
    Colon,      // :
    Percent,    // %
    Pipe,       // |
    Bang,       // !
    Question,   // ?
    At,         // @
    Dollar,     // $
    Underscore, // _
    StarStar,   // **
    ColonColon, // ::
    Arrow,      // ->
    FatArrow,   // =>
    PipeArrow,  // |>
    DotDot,     // .. for ranges
    DotDotEq,   // ..= for inclusive ranges

    // Comparison and logical operators
    NotEqual,     // !=
    Equal,        // =
    EqualEqual,   // ==
    Greater,      // >
    GreaterEqual, // >=
    Less,         // <
    LessEqual,    // <=

    // Compound assignment
    PlusEqual,    // +=
    MinusEqual,   // -=
    StarEqual,    // *=
    SlashEqual,   // /=
    PercentEqual, // %=

    // Literals
    Identifier, // Variable/function names
    Error,      // Error type name (ends with qustion mark, e.g. NetworkError! )
    String,     // "string literal"
    FString,    // f"f-string"
    RawString,  // r"raw string \n\t"
    Number,     // 123, 123.45
    Doc,        // """docstring"""

    // Keywords
    And,
    Break,
    Class,
    Const,
    Continue,
    Else,
    Enum,
    False,
    For,
    Fn,
    If,
    In,
    Match,
    Nil,
    Not,
    Or,
    Pub,
    Raise,
    Return,
    Super,
    Self_,
    True,
    Let,
    Use,
    While,

    // AI-specific keywords
    AI,
    Prompt,
    Agent,

    // Special tokens
    Invalid, // Invalid token error
    Eof,     // End of file
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

    pub fn is_function_def_keyword(&self) -> bool {
        matches!(self.kind, TokenType::Fn | TokenType::AI | TokenType::Pub)
    }

    pub fn is_error_type(&self) -> bool {
        self.kind == TokenType::Error
    }

    pub fn is_literal_token(&self) -> bool {
        matches!(
            self.kind,
            TokenType::Number
                | TokenType::String
                | TokenType::True
                | TokenType::False
                | TokenType::Nil
        )
    }

    // Whether current token is synchronize keyword,
    // this mainly used in `synchronize()`.
    // When reporing error and encounter a new synchronized keyword,
    // we should keep going to parse the next declaration.
    pub fn is_synchronize_keyword(&self) -> bool {
        matches!(
            self.kind,
            TokenType::Agent
                | TokenType::AI
                | TokenType::Class
                | TokenType::Const
                | TokenType::Enum
                | TokenType::Fn
                | TokenType::For
                | TokenType::If
                | TokenType::Let
                | TokenType::Match
                | TokenType::Pub
                | TokenType::Raise
                | TokenType::Return
                | TokenType::Use
                | TokenType::While
        )
    }

    /// Check if the token could start as an expression.
    pub fn is_expr_start(&self) -> bool {
        // Exclude TokenType::OpenBrace to avoid syntax conflict with object literal.
        matches!(
            self.kind,
            TokenType::Number
                | TokenType::String
                | TokenType::True
                | TokenType::False
                | TokenType::Nil
                | TokenType::Identifier
                | TokenType::OpenParen
                | TokenType::OpenBracket
                | TokenType::Match
                | TokenType::Minus
                | TokenType::Not
                | TokenType::Self_
                | TokenType::Super
                | TokenType::Pipe
        )
    }
}

// Lexer for tokenizing source code
struct Lexer<'a> {
    // The complete source code being scanned
    source: &'a str,
    // Character iterator for the source
    iter: Peekable<CharIndices<'a>>,
    // Start position of current token (in bytes)
    start: usize,
    // Current position in the source (in bytes)
    current: usize,
    // Current line number
    line: u32,
    // Whether we've reached the end of file
    is_eof: bool,
}

pub struct Scanner<'a> {
    lexer: peakable::Peekable<Lexer<'a>>,
    error_reporter: ErrorReporter,
    pub current: Token<'a>,
    pub previous: Token<'a>,
}

impl Deref for Scanner<'_> {
    type Target = ErrorReporter;

    fn deref(&self) -> &Self::Target {
        &self.error_reporter
    }
}

impl DerefMut for Scanner<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.error_reporter
    }
}

impl<'a> Lexer<'a> {
    // Creates a new Scanner for the given source code
    fn new(source: &'a str) -> Self {
        Self {
            source,
            iter: source.char_indices().peekable(),
            start: 0,
            current: 0,
            line: 1,
            is_eof: false,
        }
    }

    // Advances the scanner one character forward
    fn advance(&mut self) -> Option<char> {
        match self.iter.next() {
            Some((pos, ch)) => {
                self.current = pos + ch.len_utf8();
                Some(ch)
            }
            None => None,
        }
    }

    // Peeks at the next character without consuming it
    fn peek(&mut self) -> Option<char> {
        self.iter.peek().map(|&(_, c)| c)
    }

    // Looks ahead at the next N characters without consuming them
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

    // Returns the next 2 characters as a string slice
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

    // Skips whitespace and comments
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

    // Creates a token of the given type using the current lexeme
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
                    return Token::new(TokenType::Invalid, "Unterminated docstring.", self.line);
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

    // Scans numeric literals (integers and floats)
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

    // Helper method to consume consecutive digits
    fn consume_digits(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.advance();
        }
    }

    // Scans identifiers and keywords
    fn scan_identifier(&mut self) -> Token<'a> {
        while matches!(self.peek(), Some(c) if c.is_alphanumeric() || c == '_') {
            self.advance();
        }

        // Check for ! after identifier for error types
        if self.peek() == Some('!') {
            self.advance();
            // The lexeme will include the !.
            return self.make_token(TokenType::Error);
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
            "enum" => TokenType::Enum,
            "false" => TokenType::False,
            "for" => TokenType::For,
            "fn" => TokenType::Fn,
            "if" => TokenType::If,
            "in" => TokenType::In,
            "match" => TokenType::Match,
            "nil" => TokenType::Nil,
            "not" => TokenType::Not,
            "or" => TokenType::Or,
            "prompt" => TokenType::Prompt,
            "pub" => TokenType::Pub,
            "return" => TokenType::Return,
            "raise" => TokenType::Raise,
            "super" => TokenType::Super,
            "self" => TokenType::Self_,
            "true" => TokenType::True,
            "let" => TokenType::Let,
            "use" => TokenType::Use,
            "while" => TokenType::While,
            _ => TokenType::Identifier,
        };

        self.make_token(kind)
    }

    // Scans the next token from the source
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
            '@' => self.make_token(TokenType::At),
            '$' => self.make_token(TokenType::Dollar),
            '?' => self.make_token(TokenType::Question),
            '_' => {
                // Check if the next character is not alphanumeric or another underscore
                if !matches!(self.peek(), Some(c) if c.is_alphanumeric() || c == '_') {
                    self.make_token(TokenType::Underscore)
                } else {
                    // Otherwise, scan it as an identifier
                    self.scan_identifier()
                }
            }
            '.' => {
                let kind = if self.peek() == Some('.') {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        TokenType::DotDotEq
                    } else {
                        TokenType::DotDot
                    }
                } else {
                    TokenType::Dot
                };
                self.make_token(kind)
            }
            '|' => {
                let kind = if self.peek() == Some('>') {
                    self.advance();
                    TokenType::PipeArrow
                } else {
                    TokenType::Pipe
                };
                self.make_token(kind)
            }
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
            ':' => {
                let kind = if self.peek() == Some(':') {
                    self.advance();
                    TokenType::ColonColon
                } else {
                    TokenType::Colon
                };
                self.make_token(kind)
            }
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
                    TokenType::NotEqual
                } else {
                    TokenType::Bang
                };
                self.make_token(kind)
            }
            '=' => {
                let p = self.peek();
                let kind = if p == Some('=') {
                    self.advance();
                    TokenType::EqualEqual
                } else if p == Some('>') {
                    self.advance();
                    TokenType::FatArrow
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
                    self.scan_string()
                }
            }
            'f' => {
                if self.peek() == Some('"') {
                    self.scan_fstring()
                } else {
                    // If 'f' is not followed by a quote, it's just an identifier
                    self.scan_identifier()
                }
            }
            'r' => {
                // Parse raw string: r"raw string \n\t"
                if self.peek() == Some('"') {
                    self.advance(); // consume the quote
                    let mut token = self.scan_string();
                    // Change the toke type to RawString
                    token.kind = TokenType::RawString;
                    token
                } else {
                    self.scan_identifier()
                }
            }
            c if c.is_ascii_digit() => self.scan_number(),
            c if c.is_alphabetic() => self.scan_identifier(),
            _ => Token::new(TokenType::Invalid, "Unexpected character.", self.line),
        }
    }

    fn scan_fstring(&mut self) -> Token<'a> {
        // Skip the 'f' and opening quote
        self.advance(); // Skip the opening quote

        let start_content = self.current; // Where string content starts
        let mut brace_depth = 0;

        while let Some((end_pos, ch)) = self.iter.peek().copied() {
            match ch {
                '{' => {
                    brace_depth += 1;
                    self.advance();
                }
                '}' => {
                    if brace_depth > 0 {
                        brace_depth -= 1;
                    } else {
                        // Lone closing brace is treated as literal '}'
                        // This is similar to Python's f-string behavior
                        // (Could return an error here instead)
                        self.advance();
                    }
                }
                '\\' => {
                    self.advance(); // consume backslash
                    self.advance(); // consume the following char
                }
                '"' => {
                    let content = &self.source[start_content..end_pos];
                    self.advance(); // consume closing quote
                    return Token::new(TokenType::FString, content, self.line);
                }
                '\n' => {
                    self.line += 1;
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }

        Token::new(TokenType::Invalid, "Unterminated f-string.", self.line)
    }

    // Scan a string literal, handling escape sequences
    fn scan_string(&mut self) -> Token<'a> {
        let start_content = self.current; // Skip opening quote

        while let Some((end_pos, ch)) = self.iter.peek().copied() {
            match ch {
                '\\' => {
                    self.advance(); // consume backslash
                    self.advance(); // consume the following char
                }
                '"' => {
                    let content = &self.source[start_content..end_pos];
                    self.advance(); // consume closing quote
                    return Token::new(TokenType::String, content, self.line);
                }
                '\n' => {
                    self.line += 1;
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }

        Token::new(TokenType::Invalid, "Unterminated string.", self.line)
    }
}

impl Lexer<'_> {
    fn read_raw_script(&mut self) -> Result<String, String> {
        let mut script = String::new();
        let mut brace_count = 1;

        while let Some((_, ch)) = self.iter.peek() {
            match ch {
                '{' => {
                    brace_count += 1;
                    script.push('{');
                }
                '}' => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        break;
                    } else {
                        script.push('}');
                    }
                }
                '\n' => {
                    script.push('\n');
                    self.line += 1;
                }
                ch => {
                    script.push(*ch);
                }
            }
            self.advance();
        }

        if brace_count > 0 {
            return Err("Unclosed script block".to_string());
        }

        Ok(script.trim().to_owned())
    }
}

impl<'a> Iterator for Lexer<'a> {
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

impl<'a> From<peakable::Peekable<Lexer<'a>>> for Lexer<'a> {
    fn from(p: peakable::Peekable<Lexer<'a>>) -> Self {
        p.iter
    }
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Scanner {
            lexer: peakable::Peekable::new(Lexer::new(source)),
            current: Token::default(),
            previous: Token::default(),
            error_reporter: ErrorReporter::new(),
        }
    }

    pub fn read_raw_script(&mut self) -> Result<String, String> {
        let mut lexer = Lexer::from(mem::replace(
            &mut self.lexer,
            peakable::Peekable::new(Lexer::new("")),
        ));
        let script = format!("{} {}", self.current.lexeme, lexer.read_raw_script()?);

        self.lexer = peakable::Peekable::new(lexer);
        // Advance to next token.
        self.advance();
        Ok(script)
    }

    pub fn escape_string(&mut self, input: &str) -> Option<String> {
        let mut escaped_string = String::new();
        let mut chars = input.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                let c = match chars.next() {
                    Some('n') => '\n',
                    Some('r') => '\r',
                    Some('t') => '\t',
                    Some('\\') => '\\',
                    Some('\'') => '\'',
                    Some('\"') => '\"',
                    Some('0') => '\0',
                    Some(ch) => {
                        self.error(&format!("Invalid escape sequence: \\{}", ch));
                        return None;
                    }
                    None => {
                        self.error("Unterminated escape sequence");
                        return None;
                    }
                };
                escaped_string.push(c);
            } else {
                escaped_string.push(ch);
            }
        }
        Some(escaped_string)
    }

    pub fn advance(&mut self) {
        self.previous = mem::take(&mut self.current);

        while let Some(token) = self.lexer.next() {
            self.current = token;
            if self.current.kind != TokenType::Invalid {
                break;
            }
            self.error_at_current(self.current.lexeme);
        }
    }

    pub fn consume(&mut self, kind: TokenType, message: &str) {
        if self.check(kind) {
            self.advance();
            return;
        }
        self.error_at_current(message);
    }

    pub fn consume_either(&mut self, k1: TokenType, k2: TokenType, message: &str) {
        if self.check_either(k1, k2) {
            self.advance();
            return;
        }
        self.error_at_current(message);
    }

    pub fn match_token(&mut self, kind: TokenType) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn peek_next(&mut self) -> Option<Token<'a>> {
        self.lexer.peek().copied()
    }

    pub fn check(&self, kind: TokenType) -> bool {
        self.current.kind == kind
    }
    pub fn check_either(&self, k1: TokenType, k2: TokenType) -> bool {
        self.check(k1) || self.check(k2)
    }

    pub fn check_identifier(&self, lexme: &str) -> bool {
        self.current.kind == TokenType::Identifier && self.current.lexeme == lexme
    }

    pub fn check_next(&mut self, kind: TokenType) -> bool {
        self.peek_next().map(|t| t.kind == kind) == Some(true)
    }

    pub fn is_at_end(&self) -> bool {
        self.current.kind == TokenType::Eof
    }

    pub fn error_at_current(&mut self, message: &str) {
        let current = self.current;
        self.error_at(current, message);
    }

    pub fn error(&mut self, message: &str) {
        let previous = self.previous;
        self.error_at(previous, message);
    }

    pub fn synchronize(&mut self) {
        self.panic_mode = false;

        while !self.is_at_end() {
            if self.previous.kind == TokenType::Semicolon {
                return;
            }

            if self.current.is_synchronize_keyword() {
                return;
            }
            self.advance();
        }
    }
}
