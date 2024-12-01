mod ast;
mod validator;

use std::collections::HashMap;

use aiscript_lexer::{Scanner, TokenType};
use ast::Directive;

use serde_json::Value;
pub use validator::Validator;

pub struct DirectiveParser<'a, 'b: 'a> {
    scanner: &'a mut Scanner<'b>,
}

impl<'a, 'b> DirectiveParser<'a, 'b> {
    pub fn new(scanner: &'a mut Scanner<'b>) -> Self {
        if scanner.check(TokenType::Eof) {
            scanner.advance();
        }
        Self { scanner }
    }

    #[must_use]
    pub fn parse_validators(&mut self) -> Vec<Box<dyn Validator>> {
        let mut validiators = Vec::new();
        while self.scanner.check(TokenType::At) {
            if let Some(directive) = self.parse_directive() {
                validiators.push(validator::convert_from_directive(directive));
            }
        }
        validiators
    }

    #[must_use]
    pub fn parse_directive(&mut self) -> Option<Directive> {
        self.scanner
            .consume(TokenType::At, "Expected '@' at start of directive");

        // Get directive name
        if !self.scanner.is_at_end() {
            let name = self.scanner.current;
            self.scanner.advance();
            match name.kind {
                TokenType::In => self.parse_in_directive(),
                TokenType::Not => self.parse_not_directive(),
                TokenType::Identifier if name.lexeme == "any" => self.parse_any_directive(),
                TokenType::Identifier => self.parse_simple_directive(name.lexeme.to_owned()),
                _ => {
                    self.scanner.error_at_current("Expected directive name");
                    None
                }
            }
        } else {
            self.scanner.error_at_current("Unexpected end");
            None
        }
    }

    fn parse_not_directive(&mut self) -> Option<Directive> {
        self.scanner
            .consume(TokenType::OpenParen, "Expect '(' after '@not'.");
        let inner = self.parse_directive()?;
        self.scanner
            .consume(TokenType::CloseParen, "Expect ') at the end of directive.");
        Some(Directive::Not(Box::new(inner)))
    }

    fn parse_in_directive(&mut self) -> Option<Directive> {
        self.scanner
            .consume(TokenType::OpenParen, "Expect '(' after '@in'.");
        let values = self.parse_array()?;
        self.scanner
            .consume(TokenType::CloseParen, "Expect ') at the end of directive.");
        Some(Directive::In(values))
    }

    fn parse_any_directive(&mut self) -> Option<Directive> {
        self.scanner
            .consume(TokenType::OpenParen, "Expect '(' after '@any'.");
        let mut directives = Vec::new();

        while !self.scanner.check(TokenType::CloseParen) {
            directives.push(self.parse_directive()?);
            if self.scanner.check(TokenType::Comma) {
                self.scanner.advance(); // consume comma
            }
        }

        self.scanner
            .consume(TokenType::CloseParen, "Expect ') at the end of directive.");
        Some(Directive::Any(directives))
    }

    fn parse_simple_directive(&mut self, name: String) -> Option<Directive> {
        let mut params = HashMap::new();

        if self.scanner.match_token(TokenType::OpenParen) {
            while !self.scanner.check(TokenType::CloseParen) {
                self.scanner
                    .consume(TokenType::Identifier, "Expect parameter name.");
                let param_name = self.scanner.previous.lexeme.to_owned();
                self.scanner
                    .consume(TokenType::Equal, "Expect '=' after parameter.");
                let value = self.parse_value()?;
                params.insert(param_name, value);

                if self.scanner.check(TokenType::Comma) {
                    self.scanner.advance(); // consume comma
                }
            }

            self.scanner
                .consume(TokenType::CloseParen, "Expect ') at the end of directive.");
        }

        Some(Directive::Simple { name, params })
    }

    fn parse_array(&mut self) -> Option<Vec<Value>> {
        self.scanner
            .consume(TokenType::OpenBracket, "Expect '[' before array.");
        let mut values = Vec::new();

        while !self.scanner.check(TokenType::CloseBracket) {
            values.push(self.parse_value()?);

            if self.scanner.check(TokenType::Comma) {
                self.scanner.advance(); // consume comma
            }
        }

        self.scanner
            .consume(TokenType::CloseBracket, "Expect '] at the end of array.");
        Some(values)
    }

    fn parse_value(&mut self) -> Option<Value> {
        let token = self.scanner.current;
        self.scanner.advance();
        match token.kind {
            TokenType::String => Some(Value::String(token.lexeme.trim_matches('"').to_owned())),
            TokenType::Number => match token.lexeme.parse::<f64>() {
                Ok(num) => Some(Value::Number(serde_json::Number::from(num as i64))),
                Err(err) => {
                    self.scanner.error(&format!("Invalid number: {err}"));
                    None
                }
            },
            TokenType::True => Some(Value::Bool(true)),
            TokenType::False => Some(Value::Bool(false)),
            TokenType::OpenBracket => {
                let values = self.parse_array()?;
                Some(Value::Array(values))
            }
            _ => {
                self.scanner
                    .error(&format!("Unexpected token {:?}", token.kind));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_length_directive() {
        let source = "@length(max=10)";
        let mut scanner = Scanner::new(source);
        let mut parser = DirectiveParser::new(&mut scanner);
        let directive = parser.parse_directive().unwrap();

        match directive {
            Directive::Simple { name, params } => {
                assert!(!parser.scanner.had_error);
                assert_eq!(name, "length");
                assert_eq!(params.len(), 1);
                match params.get("max") {
                    Some(Value::Number(n)) => assert_eq!(*n, 10i64.into()),
                    _ => panic!("Expected max parameter with number value"),
                }
            }
            _ => panic!("Expected Simple directive"),
        }
    }

    #[test]
    fn test_not_directive() {
        let source = "@not(@another)";
        let mut scanner = Scanner::new(source);
        let mut parser = DirectiveParser::new(&mut scanner);
        let directive = parser.parse_directive().unwrap();

        match directive {
            Directive::Not(inner) => match *inner {
                Directive::Simple { name, params } => {
                    assert!(!parser.scanner.had_error);
                    assert_eq!(name, "another");
                    assert!(params.is_empty());
                }
                _ => panic!("Expected Simple directive inside Not"),
            },
            _ => panic!("Expected Not directive"),
        }
    }

    #[test]
    fn test_in_directive() {
        let source = "@in([\"a\", \"b\", \"c\"])";
        let mut scanner = Scanner::new(source);
        let mut parser = DirectiveParser::new(&mut scanner);
        let directive = parser.parse_directive().unwrap();

        match directive {
            Directive::In(values) => {
                assert!(!parser.scanner.had_error);
                assert_eq!(values.len(), 3);
                match &values[0] {
                    Value::String(s) => assert_eq!(*s, "a"),
                    _ => panic!("Expected string value"),
                }
            }
            _ => panic!("Expected In directive"),
        }
    }

    #[test]
    fn test_any_directive() {
        let source = "@any(@a, @b(arg=1), @c)";
        let mut scanner = Scanner::new(source);
        let mut parser = DirectiveParser::new(&mut scanner);
        let directive = parser.parse_directive().unwrap();

        match directive {
            Directive::Any(directives) => {
                assert!(!parser.scanner.had_error);
                assert_eq!(directives.len(), 3);
            }
            _ => panic!("Expected Any directive"),
        }
    }
}
