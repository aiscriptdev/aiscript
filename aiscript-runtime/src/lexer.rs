use std::{
    fmt::{self, Display, Formatter},
    iter::Peekable,
    str::Chars,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Endpoint Keywords
    Route,  // route
    Get,    // get
    Post,   // post
    Put,    // put
    Delete, // delete
    Query,  // query
    Body,   // body

    // Symbols
    OpenBrace,    // {
    CloseBrace,   // }
    OpenAngle,    // <
    CloseAngle,   // >
    Comma,        // ,
    Colon,        // :
    Equal,        // =
    At,           // @
    OpenParen,    // (
    CloseParen,   // )
    Slash,        // /
    OpenBracket,  // [
    CloseBracket, // ]
    Semicolon,    // ;

    // Types
    TypeStr,  // str
    TypeInt,  // int
    TypeBool, // bool

    // Values
    Identifier(String),
    StringLiteral(String),
    NumberLiteral(i64),
    BoolLiteral(bool),
    DocLine(String),
}

pub struct Lexer<'s> {
    source: &'s str,
    chars: Peekable<Chars<'s>>,
    current_pos: usize,
}

impl<'s> Lexer<'s> {
    pub fn new(source: &'s str) -> Self {
        Lexer {
            source,
            chars: source.chars().peekable(),
            current_pos: 0,
        }
    }

    fn advance(&mut self) -> Option<char> {
        self.current_pos += 1;
        self.chars.next()
    }

    fn consume_whitespace(&mut self) {
        while let Some(&ch) = self.chars.peek() {
            if ch.is_whitespace() {
                self.advance();
            }
            // Skip comments line starts with // but not ///, which is docs
            else if ch == '/' {
                if self.peek_slash(3) {
                    break;
                }

                if self.peek_slash(2) {
                    while matches!(self.chars.peek(), Some(c) if *c != '\n') {
                        self.advance();
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    fn peek_slash(&self, n: usize) -> bool {
        self.source[self.current_pos..self.current_pos + n] == "/".repeat(n)
    }

    fn read_identifier(&mut self) -> String {
        let mut identifier = String::new();

        while let Some(&ch) = self.chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                identifier.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        identifier
    }

    fn read_to_line_end(&mut self) -> String {
        let mut line = String::new();
        while let Some(&ch) = self.chars.peek() {
            if ch == '\n' {
                break;
            }
            line.push(ch);
            self.advance();
        }
        line.trim_start_matches('/').trim().to_string()
    }

    fn read_string_literal(&mut self) -> Result<String, String> {
        let mut string = String::new();
        while let Some(ch) = self.advance() {
            match ch {
                '"' => return Ok(string),
                '\\' => {
                    if let Some(next_ch) = self.advance() {
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

    pub fn read_raw_script(&mut self, first_token: &Option<Token>) -> Result<String, String> {
        let mut script = String::new();
        let mut brace_count = 1;

        // Add the first token that was already consumed if it exists
        if let Some(token) = first_token {
            script.push_str(&token.to_string());
        }

        while let Some(ch) = self.advance() {
            match ch {
                '{' => {
                    brace_count += 1;
                    script.push(ch);
                }
                '}' => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        // Don't include the final closing brace
                        break;
                    } else {
                        script.push(ch);
                    }
                }
                ch => script.push(ch),
            }
        }

        if brace_count > 0 {
            return Err("Unclosed script block".to_string());
        }

        Ok(script.trim().to_owned())
    }

    fn parse_token(&mut self, ch: char) -> Result<Token, String> {
        match ch {
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
            '[' => Ok(Token::OpenBracket),
            ']' => Ok(Token::CloseBracket),
            ';' => Ok(Token::Semicolon),
            '"' => self.read_string_literal().map(Token::StringLiteral),
            '/' => {
                if self.peek_slash(2) {
                    Ok(Token::DocLine(self.read_to_line_end()))
                } else {
                    Ok(Token::Slash)
                }
            }
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
                while let Some(&ch) = self.chars.peek() {
                    if ch.is_numeric() {
                        num.push(ch);
                        self.advance();
                    } else {
                        break;
                    }
                }
                num.parse::<i64>()
                    .map(Token::NumberLiteral)
                    .map_err(|e| e.to_string())
            }
            ch => Err(format!("Unexpected character: {}", ch)),
        }
    }

    pub fn next_token(&mut self) -> Option<Result<Token, String>> {
        self.consume_whitespace();
        let next = self.advance()?;
        Some(self.parse_token(next))
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Token::Route => write!(f, "route"),
            Token::Get => write!(f, "get"),
            Token::Post => write!(f, "post"),
            Token::Put => write!(f, "put"),
            Token::Delete => write!(f, "delete"),
            Token::Query => write!(f, "query"),
            Token::Body => write!(f, "body"),
            Token::OpenBrace => write!(f, "{{"),
            Token::CloseBrace => write!(f, "}}"),
            Token::OpenAngle => write!(f, "<"),
            Token::CloseAngle => write!(f, ">"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::Equal => write!(f, "="),
            Token::At => write!(f, "@"),
            Token::OpenParen => write!(f, "("),
            Token::CloseParen => write!(f, ")"),
            Token::Slash => write!(f, "/"),
            Token::OpenBracket => write!(f, "["),
            Token::CloseBracket => write!(f, "]"),
            Token::Semicolon => write!(f, ";"),
            Token::TypeStr => write!(f, "str"),
            Token::TypeInt => write!(f, "int"),
            Token::TypeBool => write!(f, "bool"),
            Token::Identifier(ident) => write!(f, "{}", ident),
            Token::StringLiteral(s) => write!(f, "{}", s),
            Token::NumberLiteral(n) => write!(f, "{}", n),
            Token::BoolLiteral(b) => write!(f, "{}", b),
            Token::DocLine(s) => write!(f, "/// {}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lexer() {
        let input = r#"
            /// Users API line1
            /// Users API line2
            route /api/users {
                /// Test endpoint
                get /<id: int>, put /, post /, delete / {
                    query {
                        /// Field name
                        @string(max_len=10)
                        name: str = "John"
                        flag: bool = true
                    }

                    @form
                    body {
                        @any(@number(min=100), @is_admin))
                        id: int = 1
                        test: bool = false
                        @in(["a", "b", "c"])
                        choice: str = "a"
                    }

                    // comment should be ignored
                }

                /// Test endpoint
                get /b {
                    return "endpoint b";
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
                "DocLine(\"Users API line1\")",
                "DocLine(\"Users API line2\")",
                // route
                "Route",
                "Slash",
                "Identifier(\"api\")",
                "Slash",
                "Identifier(\"users\")",
                "OpenBrace",
                // get
                "DocLine(\"Test endpoint\")",
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
                "DocLine(\"Field name\")",
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
                "StringLiteral(\"John\")",
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
                // @in(["a", "b", "c"])
                "At",
                "Identifier(\"in\")",
                "OpenParen",
                "OpenBracket",
                "StringLiteral(\"a\")",
                "Comma",
                "StringLiteral(\"b\")",
                "Comma",
                "StringLiteral(\"c\")",
                "CloseBracket",
                "CloseParen",
                // choice: str = "a"
                "Identifier(\"choice\")",
                "Colon",
                "TypeStr",
                "Equal",
                "StringLiteral(\"a\")",
                "CloseBrace",
                "CloseBrace",
                // Test endpoint
                "DocLine(\"Test endpoint\")",
                // get /b
                "Get",
                "Slash",
                "Identifier(\"b\")",
                "OpenBrace",
                // return "endpoint b"
                "Identifier(\"return\")",
                "StringLiteral(\"endpoint b\")",
                "Semicolon",
                "CloseBrace",
                "CloseBrace",
            ]
        );
    }
}
