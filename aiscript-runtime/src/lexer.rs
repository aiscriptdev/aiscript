use std::{iter::Peekable, str::CharIndices};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Route,
    Get,
    Post,
    Put,
    Delete,
    Query,
    Body,

    // Symbols
    OpenBrace,  // {
    CloseBrace, // }
    OpenAngle,  // <
    CloseAngle, // >
    Comma,      // ,
    Colon,      // :
    Equal,      // =
    At,         // @
    OpenParen,  // (
    CloseParen, // )
    Slash,      // /

    // Types
    TypeStr,
    TypeInt,
    TypeBool,

    // Values
    Identifier(String),
    StringLiteral(String),
    NumberLiteral(i64),
    BoolLiteral(bool),

    // Special token for raw script content
    RawScript(String),
}

#[derive(Debug)]
pub enum LexerMode {
    Normal,
    RawScript { brace_count: i32 },
}

pub struct Lexer<'input> {
    input: &'input str,
    chars: Peekable<CharIndices<'input>>,
    mode: LexerMode,
    current_pos: usize,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Lexer {
            input,
            chars: input.char_indices().peekable(),
            mode: LexerMode::Normal,
            current_pos: 0,
        }
    }

    fn consume_whitespace(&mut self) {
        while let Some(&(_, ch)) = self.chars.peek() {
            if ch.is_whitespace() {
                self.chars.next();
            } else {
                break;
            }
        }
    }

    fn read_identifier(&mut self) -> String {
        let mut identifier = String::new();

        while let Some(&(_, ch)) = self.chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                identifier.push(ch);
                self.chars.next();
            } else {
                break;
            }
        }

        identifier
    }

    fn read_string_literal(&mut self) -> Result<String, String> {
        let mut string = String::new();
        self.chars.next(); // Skip opening quote

        while let Some((_, ch)) = self.chars.next() {
            match ch {
                '"' => return Ok(string),
                '\\' => {
                    if let Some((_, next_ch)) = self.chars.next() {
                        string.push(match next_ch {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '\\' => '\\',
                            '"' => '"',
                            _ => return Err(format!("Invalid escape sequence: \\{}", next_ch)),
                        });
                    }
                }
                _ => string.push(ch),
            }
        }

        Err("Unterminated string literal".to_string())
    }

    fn read_raw_script(&mut self) -> Result<String, String> {
        let mut script = String::new();
        let mut brace_count = 1;

        while let Some((_, ch)) = self.chars.next() {
            match ch {
                '{' => brace_count += 1,
                '}' => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        return Ok(script);
                    }
                }
                _ => {}
            }
            script.push(ch);
        }

        Err("Unclosed script block".to_string())
    }

    pub fn next_token(&mut self) -> Option<Result<Token, String>> {
        self.consume_whitespace();

        match self.mode {
            LexerMode::RawScript { .. } => match self.read_raw_script() {
                Ok(script) => {
                    self.mode = LexerMode::Normal;
                    Some(Ok(Token::RawScript(script)))
                }
                Err(e) => Some(Err(e)),
            },
            LexerMode::Normal => match self.chars.next() {
                Some((_, ch)) => Some(match ch {
                    '{' => Ok(Token::OpenBrace),
                    '}' => Ok(Token::CloseBrace),
                    '<' => Ok(Token::OpenAngle),
                    '>' => Ok(Token::CloseAngle),
                    ',' => Ok(Token::Comma),
                    ':' => Ok(Token::Colon),
                    '=' => Ok(Token::Equal),
                    '@' => Ok(Token::At),
                    '(' => Ok(Token::OpenParen),
                    ')' => Ok(Token::CloseParen),
                    '/' => Ok(Token::Slash),
                    '"' => self.read_string_literal().map(Token::StringLiteral),
                    ch if ch.is_alphabetic() => {
                        let mut ident = ch.to_string();
                        ident.push_str(&self.read_identifier());

                        Ok(match ident.as_str() {
                            "route" => Token::Route,
                            "get" => Token::Get,
                            "post" => Token::Post,
                            "put" => Token::Put,
                            "delete" => Token::Delete,
                            "query" => Token::Query,
                            "body" => Token::Body,
                            "str" => Token::TypeStr,
                            "int" => Token::TypeInt,
                            "bool" => Token::TypeBool,
                            "true" => Token::BoolLiteral(true),
                            "false" => Token::BoolLiteral(false),
                            _ => Token::Identifier(ident),
                        })
                    }
                    ch if ch.is_numeric() || ch == '-' => {
                        let mut num = ch.to_string();
                        while let Some(&(_, ch)) = self.chars.peek() {
                            if ch.is_numeric() {
                                num.push(ch);
                                self.chars.next();
                            } else {
                                break;
                            }
                        }
                        num.parse::<i64>()
                            .map(Token::NumberLiteral)
                            .map_err(|e| e.to_string())
                    }
                    ch => Err(format!("Unexpected character: {}", ch)),
                }),
                None => None,
            },
        }
    }

    pub fn switch_to_raw_script(&mut self) {
        self.mode = LexerMode::RawScript { brace_count: 1 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lexer() {
        let input = r#"
            route /api/users {
                get /<id: int>, put /, post /, delete / {
                    query {
                        @string(max_len=10)
                        name: str = "John"
                        flag: bool = true
                    }

                    @form
                    body {
                        @any(@number(min=100), @is_admin))
                        id: int = 1
                        test: bool = false
                    }
                }
            }"#;
        let mut lexer = Lexer::new(input);
        let mut tokens = Vec::new();
        while let Some(token) = lexer.next_token() {
            tokens.push(token);
        }
        assert_eq!(
            tokens
                .into_iter()
                .map(|token| format!("{:?}", token.unwrap()))
                .collect::<Vec<String>>(),
            vec![
                "Route",
                "Slash",
                "Identifier(\"api\")",
                "Slash",
                "Identifier(\"users\")",
                "OpenBrace",
                // get
                "Get",
                "Slash",
                "OpenAngle",
                "Identifier(\"id\")",
                "Colon",
                "TypeInt",
                "CloseAngle",
                "Comma",
                // put
                "Put",
                "Slash",
                "Comma",
                // post
                "Post",
                "Slash",
                "Comma",
                // delete
                "Delete",
                "Slash",
                "OpenBrace",
                // query
                "Query",
                "OpenBrace",
                // @string(max_len=10)
                "At",
                "Identifier(\"string\")",
                "OpenParen",
                "Identifier(\"max_len\")",
                "Equal",
                "NumberLiteral(10)",
                "CloseParen",
                // name: str = "John"
                "Identifier(\"name\")",
                "Colon",
                "TypeStr",
                "Equal",
                "StringLiteral(\"ohn\")",
                // flag: bool = true
                "Identifier(\"flag\")",
                "Colon",
                "TypeBool",
                "Equal",
                "BoolLiteral(true)",
                "CloseBrace",
                // @form
                "At",
                "Identifier(\"form\")",
                // body
                "Body",
                "OpenBrace",
                // @any
                "At",
                "Identifier(\"any\")",
                "OpenParen",
                // @number(min=100)
                "At",
                "Identifier(\"number\")",
                "OpenParen",
                "Identifier(\"min\")",
                "Equal",
                "NumberLiteral(100)",
                "CloseParen",
                "Comma",
                // @is_admin
                "At",
                "Identifier(\"is_admin\")",
                "CloseParen",
                "CloseParen",
                // id: int = 1
                "Identifier(\"id\")",
                "Colon",
                "TypeInt",
                "Equal",
                "NumberLiteral(1)",
                // test: bool = false
                "Identifier(\"test\")",
                "Colon",
                "TypeBool",
                "Equal",
                "BoolLiteral(false)",
                "CloseBrace",
                "CloseBrace",
                "CloseBrace",
            ]
        );
    }
}
