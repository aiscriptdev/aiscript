use std::{iter::Peekable, str::Chars};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    OpenParen,  // (
    CloseParen, // )
    OpenBrace,  // {
    CloseBrace, // }
    Comma,      // ,
    Dot,        // .
    Minus,      // -
    Plus,       // +
    Semicolon,  // ;
    Slash,      // /
    Star,       // *

    Bang,         // !
    BangEqual,    // !=
    Equal,        // =
    EqualEqual,   // ==
    Greater,      // >
    GreaterEqual, // >=
    Less,         // <
    LessEqual,    // <=

    Identifier,
    String,
    Number,

    // Normal keywords
    And,    // and
    Class,  // class
    Else,   // else
    False,  // false
    For,    // for
    Fn,     // fn
    If,     // if
    Nil,    // nil
    Or,     // or
    Print,  // print
    Return, // return
    Super,  // super
    This,   // this
    True,   // true
    Let,    // let
    While,  // while

    // AI keywords
    AI,     // ai
    Prompt, // prompt
    Agent,  // agent

    Error,
    Eof,
}

#[derive(Debug, Copy, Clone)]
pub struct Token<'a> {
    pub lexeme: &'a str,
    pub line: u32,
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
    pub fn new(kind: TokenType, origin: &'a str, line: u32) -> Self {
        Token {
            kind,
            lexeme: origin,
            line,
        }
    }

    #[cfg(feature = "v1")]
    pub fn identifier(name: &'a str) -> Self {
        Token::new(TokenType::Identifier, name, 0)
    }
}

pub struct Scanner<'a> {
    pub source: &'a str,
    iter: Peekable<Chars<'a>>,
    pub start: usize,
    pub current: usize,
    pub line: u32,
    is_eof: bool,
}

impl<'a> Scanner<'a> {
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

    fn advance(&mut self) -> Option<char> {
        self.current += 1;
        let len = self.source.len();
        // Skip characters that are not valid UTF-8 characters.
        while !self.source.is_char_boundary(self.current) && self.current < len {
            self.current += 1;
        }
        self.iter.next()
    }

    fn peek(&mut self) -> Option<&char> {
        self.iter.peek()
    }

    fn next2(&mut self) -> &str {
        &self.source[self.current..=self.current + 1]
    }

    fn peek2(&mut self) -> &str {
        &self.source[self.current - 1..=self.current]
    }

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

    fn make_token(&self, kind: TokenType) -> Token<'a> {
        Token {
            kind,
            lexeme: &self.source[self.start..self.current],
            line: self.line,
        }
    }

    fn _advance_digit(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.advance();
        }
    }

    fn scan_number(&mut self) -> Token<'a> {
        // TODO: fix this: 0p =>  19 Number 0
        //                         | Identifier p
        self._advance_digit();
        // Look for a fractional part.
        if self.source[self.current..].len() >= 2 {
            let mut next_two_chars = self.source[self.current..self.current + 2].chars();
            let (maybe_dot, maybe_digit) = (next_two_chars.next(), next_two_chars.next());
            if maybe_dot == Some('.') && matches!(maybe_digit, Some(c) if c.is_ascii_digit()) {
                // Consume the "."
                self.advance();

                self._advance_digit();
            }
        }

        self.make_token(TokenType::Number)
    }

    fn scan_identifier(&mut self) -> Token<'a> {
        while matches!(self.peek(), Some(c) if c.is_alphanumeric() || *c == '_') {
            self.advance();
        }

        let kind = match &self.source[self.start..self.current] {
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

    fn scan_token(&mut self) -> Token<'a> {
        self.skip_white_spaces();

        self.start = self.current;
        if let Some(c) = self.advance() {
            if c.is_ascii_digit() {
                return self.scan_number();
            }

            if c.is_alphabetic() || c == '_' {
                return self.scan_identifier();
            }

            match c {
                '(' => return self.make_token(TokenType::OpenParen),
                ')' => return self.make_token(TokenType::CloseParen),
                '{' => return self.make_token(TokenType::OpenBrace),
                '}' => return self.make_token(TokenType::CloseBrace),
                ';' => return self.make_token(TokenType::Semicolon),
                ',' => return self.make_token(TokenType::Comma),
                '.' => return self.make_token(TokenType::Dot),
                '-' => return self.make_token(TokenType::Minus),
                '+' => return self.make_token(TokenType::Plus),
                '/' => return self.make_token(TokenType::Slash),
                '*' => return self.make_token(TokenType::Star),
                '!' => {
                    return if self.peek2() == "!=" {
                        self.advance();
                        self.make_token(TokenType::BangEqual)
                    } else {
                        self.make_token(TokenType::Bang)
                    };
                }
                '=' => {
                    return if self.peek2() == "==" {
                        self.advance();
                        self.make_token(TokenType::EqualEqual)
                    } else {
                        self.make_token(TokenType::Equal)
                    };
                }
                '<' => {
                    return if self.peek2() == "<=" {
                        self.advance();
                        self.make_token(TokenType::LessEqual)
                    } else {
                        self.make_token(TokenType::Less)
                    };
                }
                '>' => {
                    return if self.peek2() == ">=" {
                        self.advance();
                        self.make_token(TokenType::GreaterEqual)
                    } else {
                        self.make_token(TokenType::Greater)
                    };
                }
                '"' => {
                    while let Some(&ch) = self.peek() {
                        if ch == '"' {
                            break;
                        }
                        if ch == '\n' {
                            self.line += 1;
                        }
                        self.advance();
                    }

                    if self.peek().is_none() {
                        return Token::new(TokenType::Error, "Unterminated string.", self.line);
                    }

                    self.advance();
                    return self.make_token(TokenType::String);
                }
                _ => {
                    return Token::new(TokenType::Error, "Unexpected character.", self.line);
                }
            }
        }

        Token::new(TokenType::Eof, "", self.line)
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
