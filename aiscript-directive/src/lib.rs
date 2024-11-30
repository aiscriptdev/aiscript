mod ast;
mod validator;

use std::collections::HashMap;

use aiscript_lexer::{Scanner, TokenType};
// use ast::Directive;

#[derive(Debug, Clone)]
pub enum DirectiveValue<'a> {
    String(&'a str),
    Number(f64),
    Boolean(bool),
    Array(Vec<DirectiveValue<'a>>),
}

#[derive(Debug, Clone)]
pub enum Directive<'a> {
    Not(Box<Directive<'a>>),
    In(Vec<DirectiveValue<'a>>),
    Any(Vec<Directive<'a>>),
    Simple {
        name: &'a str,
        params: HashMap<&'a str, DirectiveValue<'a>>,
    },
}

pub struct DirectiveParser<'a> {
    scanner: &'a mut Scanner<'a>,
}

impl<'a> DirectiveParser<'a> {
    pub fn new(scanner: &'a mut Scanner<'a>) -> Self {
        scanner.advance();
        Self { scanner }
    }

    pub fn parse_directive(&mut self) -> Result<Directive<'a>, String> {
        self.scanner
            .consume(TokenType::At, "Expected '@' at start of directive");

        // Get directive name
        if !self.scanner.is_at_end() {
            let name = self.scanner.current;
            self.scanner.advance();
            match name.kind {
                TokenType::In => self.parse_in_directive(),
                TokenType::Identifier if name.lexeme == "not" => self.parse_not_directive(),
                TokenType::Identifier if name.lexeme == "any" => self.parse_any_directive(),
                TokenType::Identifier => self.parse_simple_directive(name.lexeme),
                _ => Err("Expected directive name".to_string()),
            }
        } else {
            Err("Unexpected end".to_string())
        }
    }

    fn parse_not_directive(&mut self) -> Result<Directive<'a>, String> {
        self.scanner
            .consume(TokenType::OpenParen, "Expect '(' after '@not'.");
        let inner = self.parse_directive()?;
        self.scanner
            .consume(TokenType::CloseParen, "Expect ') at the end of directive.");
        Ok(Directive::Not(Box::new(inner)))
    }

    fn parse_in_directive(&mut self) -> Result<Directive<'a>, String> {
        self.scanner
            .consume(TokenType::OpenParen, "Expect '(' after '@in'.");
        let values = self.parse_array()?;
        self.scanner
            .consume(TokenType::CloseParen, "Expect ') at the end of directive.");
        Ok(Directive::In(values))
    }

    fn parse_any_directive(&mut self) -> Result<Directive<'a>, String> {
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
        Ok(Directive::Any(directives))
    }

    fn parse_simple_directive(&mut self, name: &'a str) -> Result<Directive<'a>, String> {
        let mut params = HashMap::new();

        if self.scanner.match_token(TokenType::OpenParen) {
            while !self.scanner.check(TokenType::CloseParen) {
                self.scanner
                    .consume(TokenType::Identifier, "Expect parameter name.");
                let param_name = self.scanner.previous.lexeme;
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

        Ok(Directive::Simple { name, params })
    }

    fn parse_array(&mut self) -> Result<Vec<DirectiveValue<'a>>, String> {
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
        Ok(values)
    }

    fn parse_value(&mut self) -> Result<DirectiveValue<'a>, String> {
        let token = self.scanner.current;
        self.scanner.advance();
        match token.kind {
            TokenType::String => Ok(DirectiveValue::String(token.lexeme.trim_matches('"'))),
            TokenType::Number => {
                let num = token
                    .lexeme
                    .parse::<f64>()
                    .map_err(|_| "Invalid number".to_string())?;
                Ok(DirectiveValue::Number(num))
            }
            TokenType::True => Ok(DirectiveValue::Boolean(true)),
            TokenType::False => Ok(DirectiveValue::Boolean(false)),
            TokenType::OpenBracket => {
                let values = self.parse_array()?;
                Ok(DirectiveValue::Array(values))
            }
            _ => Err(format!("Unexpected token {:?}", token.kind)),
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
                    Some(DirectiveValue::Number(n)) => assert_eq!(*n, 10.0),
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
                    DirectiveValue::String(s) => assert_eq!(*s, "a"),
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
