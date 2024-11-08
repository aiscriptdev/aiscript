use std::{iter::Peekable, str::Chars};

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

    // Literals
    Identifier, // Variable/function names
    String,     // "string literal"
    Number,     // 123, 123.45
    Doc,        // """docstring"""

    // Keywords
    And,
    Class,
    Else,
    False,
    For,
    Fn,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Let,
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
    iter: Peekable<Chars<'a>>,
    /// Start position of current token
    pub start: usize,
    /// Current position in the source
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
            iter: source.chars().peekable(),
            start: 0,
            current: 0,
            line: 1,
            is_eof: false,
        }
    }

    /// Advances the scanner one character forward
    fn advance(&mut self) -> Option<char> {
        self.current += 1;
        // Handle UTF-8 character boundaries correctly
        while !self.source.is_char_boundary(self.current) && self.current < self.source.len() {
            self.current += 1;
        }
        self.iter.next()
    }

    /// Peeks at the next character without consuming it
    fn peek(&mut self) -> Option<&char> {
        self.iter.peek()
    }

    /// Looks ahead at the next N characters without consuming them
    fn check_next(&self, n: usize) -> Option<&str> {
        if self.current + n <= self.source.len() {
            Some(&self.source[self.current..self.current + n])
        } else {
            None
        }
    }

    /// Returns the next 2 characters as a string slice
    fn next2(&mut self) -> &str {
        &self.source[self.current..=self.current + 1]
    }

    /// Returns the previous and current character as a string slice
    fn peek2(&mut self) -> &str {
        &self.source[self.current - 1..=self.current]
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
                        while matches!(self.peek(), Some(c) if *c != '\n') {
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

    /// Scans a docstring token
    fn scan_docstring(&mut self) -> Token<'a> {
        // Skip the opening quotes
        for _ in 0..3 {
            self.advance();
        }

        self.start = self.current;

        loop {
            // Check for EOF
            if self.current >= self.source.len() {
                return Token::new(TokenType::Error, "Unterminated docstring.", self.line);
            }

            // Get current character
            let current_char = self.source[self.current..].chars().next().unwrap();

            if current_char == '"' {
                // Check for closing triple quotes
                if let Some("\"\"\"") = self.check_next(3) {
                    let token = self.make_token(TokenType::Doc);
                    // Skip the closing quotes
                    for _ in 0..3 {
                        self.advance();
                    }
                    return token;
                }
            }

            if current_char == '\n' {
                self.line += 1;
            }

            self.advance();
        }
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
        while matches!(self.peek(), Some(c) if c.is_alphanumeric() || *c == '_') {
            self.advance();
        }

        let text = &self.source[self.start..self.current];
        let kind = match text {
            "ai" => TokenType::AI,
            "agent" => TokenType::Agent,
            "and" => TokenType::And,
            "class" => TokenType::Class,
            "else" => TokenType::Else,
            "false" => TokenType::False,
            "for" => TokenType::For,
            "fn" => TokenType::Fn,
            "if" => TokenType::If,
            "nil" => TokenType::Nil,
            "or" => TokenType::Or,
            "print" => TokenType::Print,
            "prompt" => TokenType::Prompt,
            "return" => TokenType::Return,
            "super" => TokenType::Super,
            "this" => TokenType::This,
            "true" => TokenType::True,
            "let" => TokenType::Let,
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
                if self.peek() == Some(&'>') {
                    self.advance();
                    self.make_token(TokenType::Arrow)
                } else {
                    self.make_token(TokenType::Minus)
                }
            }
            '+' => self.make_token(TokenType::Plus),
            '/' => self.make_token(TokenType::Slash),
            '*' => self.make_token(TokenType::Star),
            ':' => self.make_token(TokenType::Colon),
            '!' => {
                let kind = if self.peek2() == "!=" {
                    self.advance();
                    TokenType::BangEqual
                } else {
                    TokenType::Bang
                };
                self.make_token(kind)
            }
            '=' => {
                let kind = if self.peek2() == "==" {
                    self.advance();
                    TokenType::EqualEqual
                } else {
                    TokenType::Equal
                };
                self.make_token(kind)
            }
            '<' => {
                let kind = if self.peek2() == "<=" {
                    self.advance();
                    TokenType::LessEqual
                } else {
                    TokenType::Less
                };
                self.make_token(kind)
            }
            '>' => {
                let kind = if self.peek2() == ">=" {
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
                    while let Some(&ch) = self.peek() {
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
                        Some(&'"') => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docstring() {
        let source = r#"fn test() {
    """
    This is a
    multiline docstring
    """
    print("Hello");
}"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let doc_token = tokens
            .iter()
            .find(|t| t.kind == TokenType::Doc)
            .expect("Docstring token not found");

        assert_eq!(
            doc_token.lexeme.trim(),
            "This is a\n    multiline docstring"
        );
    }

    #[test]
    fn test_unterminated_docstring() {
        let source = r#"fn test() {
    """
    Unterminated docstring
    "#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let error_token = tokens
            .iter()
            .find(|t| t.kind == TokenType::Error)
            .expect("Error token not found");

        assert_eq!(error_token.lexeme, "Unterminated docstring.");
    }

    #[test]
    fn test_string_tokens() {
        let source = r#"print("Hello" "World");"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        // Verify the sequence of tokens
        assert_eq!(tokens[0].kind, TokenType::Print);
        assert_eq!(tokens[1].kind, TokenType::OpenParen);
        assert_eq!(tokens[2].kind, TokenType::String);
        assert_eq!(tokens[3].kind, TokenType::String);
        assert_eq!(tokens[4].kind, TokenType::CloseParen);
        assert_eq!(tokens[5].kind, TokenType::Semicolon);

        // Verify the string contents
        assert_eq!(tokens[2].lexeme, "\"Hello\"");
        assert_eq!(tokens[3].lexeme, "\"World\"");
    }

    #[test]
    fn test_numbers() {
        let source = "123 123.456 0.1";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        // Only check the number tokens, ignoring whitespace
        let number_tokens: Vec<&Token> = tokens
            .iter()
            .filter(|t| t.kind == TokenType::Number)
            .collect();

        assert_eq!(number_tokens.len(), 3);
        assert_eq!(number_tokens[0].lexeme, "123");
        assert_eq!(number_tokens[1].lexeme, "123.456");
        assert_eq!(number_tokens[2].lexeme, "0.1");
    }

    #[test]
    fn test_keywords() {
        let source = "fn let if else while for";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let keywords: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            keywords,
            vec![
                TokenType::Fn,
                TokenType::Let,
                TokenType::If,
                TokenType::Else,
                TokenType::While,
                TokenType::For,
            ]
        );
    }

    #[test]
    fn test_operators() {
        let source = "+ - * / >= <= == != ->";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let operators: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            operators,
            vec![
                TokenType::Plus,
                TokenType::Minus,
                TokenType::Star,
                TokenType::Slash,
                TokenType::GreaterEqual,
                TokenType::LessEqual,
                TokenType::EqualEqual,
                TokenType::BangEqual,
                TokenType::Arrow,
            ]
        );
    }

    #[test]
    fn test_line_counting() {
        let source = "line1\nline2\n\nline4";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        // Find identifiers and check their line numbers
        let identifiers: Vec<(String, u32)> = tokens
            .iter()
            .filter(|t| t.kind == TokenType::Identifier)
            .map(|t| (t.lexeme.to_string(), t.line))
            .collect();

        assert_eq!(
            identifiers,
            vec![
                ("line1".to_string(), 1),
                ("line2".to_string(), 2),
                ("line4".to_string(), 4),
            ]
        );
    }

    #[test]
    fn test_comments() {
        let source = r#"// This is a comment
fn test() { // Another comment
    print("Hello"); // Comment after code
}"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let token_types: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            token_types,
            vec![
                TokenType::Fn,
                TokenType::Identifier,
                TokenType::OpenParen,
                TokenType::CloseParen,
                TokenType::OpenBrace,
                TokenType::Print,
                TokenType::OpenParen,
                TokenType::String,
                TokenType::CloseParen,
                TokenType::Semicolon,
                TokenType::CloseBrace,
            ]
        );
    }

    #[test]
    fn test_ai_keywords() {
        let source = "ai agent prompt";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let keywords: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            keywords,
            vec![TokenType::AI, TokenType::Agent, TokenType::Prompt,]
        );
    }

    #[test]
    fn test_mixed_tokens() {
        let source = r#"fn calculate(x: number) {
    let result = x * 2;
    return result;
}"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let token_types: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            token_types,
            vec![
                TokenType::Fn,
                TokenType::Identifier, // calculate
                TokenType::OpenParen,
                TokenType::Identifier, // x
                TokenType::Colon,
                TokenType::Identifier, // number
                TokenType::CloseParen,
                TokenType::OpenBrace,
                TokenType::Let,
                TokenType::Identifier, // result
                TokenType::Equal,
                TokenType::Identifier, // x
                TokenType::Star,
                TokenType::Number, // 2
                TokenType::Semicolon,
                TokenType::Return,
                TokenType::Identifier, // result
                TokenType::Semicolon,
                TokenType::CloseBrace,
            ]
        );
    }
}
