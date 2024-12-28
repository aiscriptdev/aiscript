pub mod validator;

use std::collections::HashMap;

use aiscript_lexer::{Scanner, TokenType};

use serde_json::Value;
pub use validator::Validator;

pub trait FromDirective {
    fn from_directive(directive: Directive) -> Result<Self, String>
    where
        Self: Sized;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Directive {
    pub name: String,
    pub params: DirectiveParams,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DirectiveParams {
    KeyValue(HashMap<String, Value>),
    Array(Vec<Value>),
    Directives(Vec<Directive>),
}

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
        self.parse_directives()
            .into_iter()
            .filter_map(|directive| match FromDirective::from_directive(directive) {
                Ok(validator) => Some(validator),
                Err(err) => {
                    self.scanner.error(&err);
                    None
                }
            })
            .collect()
    }

    #[must_use]
    pub fn parse_directives(&mut self) -> Vec<Directive> {
        let mut directives = Vec::new();
        while self.scanner.check(TokenType::At) {
            if let Some(directive) = self.parse_directive() {
                directives.push(directive);
            }
        }
        directives
    }

    #[must_use]
    pub fn parse_directive(&mut self) -> Option<Directive> {
        self.scanner
            .consume(TokenType::At, "Expected '@' at start of directive");

        if self.scanner.is_at_end() {
            self.scanner.error_at_current("Unexpected end");
            return None;
        }

        let name_token = self.scanner.current;
        self.scanner.advance();
        let name = name_token.lexeme.to_owned();

        let params = if self.scanner.match_token(TokenType::OpenParen) {
            let params = self.parse_parameters()?;
            self.scanner
                .consume(TokenType::CloseParen, "Expect ')' after parameters.");
            params
        } else {
            DirectiveParams::KeyValue(HashMap::new())
        };

        Some(Directive { name, params })
    }

    fn parse_parameters(&mut self) -> Option<DirectiveParams> {
        // Handle empty parentheses case first
        if self.scanner.check(TokenType::CloseParen) {
            return Some(DirectiveParams::KeyValue(HashMap::new()));
        }

        if self.scanner.check(TokenType::OpenBracket) {
            // Parse array
            let array = self.parse_array()?;
            Some(DirectiveParams::Array(array))
        } else if self.scanner.check(TokenType::At) {
            // Parse one or more directives separated by commas
            // self.scanner.advance(); // consume '@'
            let mut directives = Vec::new();
            loop {
                if let Some(directive) = self.parse_directive() {
                    directives.push(directive);
                }
                if !self.scanner.check(TokenType::Comma) {
                    break;
                }
                self.scanner.advance(); // consume ','
            }
            Some(DirectiveParams::Directives(directives))
        } else if self.scanner.check(TokenType::Identifier) {
            // Parse key-value parameters
            let mut params = HashMap::new();
            while !self.scanner.check(TokenType::CloseParen) {
                self.scanner
                    .consume(TokenType::Identifier, "Expect parameter key.");
                let key = self.scanner.previous.lexeme.to_owned();
                self.scanner
                    .consume(TokenType::Equal, "Expect '=' after parameter key.");
                let value = self.parse_value()?;
                params.insert(key, value);
                if !self.scanner.check(TokenType::Comma) {
                    break;
                }
                self.scanner.advance(); // consume ','
            }
            Some(DirectiveParams::KeyValue(params))
        } else {
            self.scanner.error("Expected parameters.");
            None
        }
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
            TokenType::String => Some(Value::String(token.lexeme.to_owned())),
            TokenType::Number => {
                let num_str = token.lexeme;
                // First try parsing as i64 (integer)
                if let Ok(int_val) = num_str.parse::<i64>() {
                    Some(Value::Number(serde_json::Number::from(int_val)))
                } else {
                    // If not an integer, try as f64 (float)
                    match num_str.parse::<f64>() {
                        Ok(float_val) => match serde_json::Number::from_f64(float_val) {
                            Some(num) => Some(Value::Number(num)),
                            None => {
                                self.scanner.error("Invalid float value");
                                None
                            }
                        },
                        Err(err) => {
                            self.scanner.error(&format!("Invalid number: {err}"));
                            None
                        }
                    }
                }
            }
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
    use aiscript_lexer::Scanner;
    use serde_json::json;

    fn parse_single_directive(input: &str) -> Option<Directive> {
        let mut scanner = Scanner::new(input);
        let mut parser = DirectiveParser::new(&mut scanner);
        parser.parse_directive()
    }

    #[test]
    fn test_simple_directive() {
        let directive = parse_single_directive("@validate").unwrap();
        assert_eq!(directive.name, "validate");
        assert!(matches!(directive.params, DirectiveParams::KeyValue(ref map) if map.is_empty()));
    }

    #[test]
    fn test_directive_with_array() {
        let directive = parse_single_directive("@values([1, 2, 3])").unwrap();
        assert_eq!(directive.name, "values");
        match directive.params {
            DirectiveParams::Array(values) => {
                assert_eq!(values.len(), 3);
                assert_eq!(values[0], json!(1));
                assert_eq!(values[1], json!(2));
                assert_eq!(values[2], json!(3));
            }
            _ => panic!("Expected Array parameters"),
        }
    }

    #[test]
    fn test_directive_with_mixed_array() {
        let directive = parse_single_directive(r#"@values([1, "test", true])"#).unwrap();
        assert_eq!(directive.name, "values");
        match directive.params {
            DirectiveParams::Array(values) => {
                assert_eq!(values.len(), 3);
                assert_eq!(values[0], json!(1));
                assert_eq!(values[1], json!("test"));
                assert_eq!(values[2], json!(true));
            }
            _ => panic!("Expected Array parameters"),
        }
    }

    #[test]
    fn test_directive_with_key_value() {
        let directive = parse_single_directive(r#"@validate(min=1, max=10, name="test")"#).unwrap();
        assert_eq!(directive.name, "validate");
        match directive.params {
            DirectiveParams::KeyValue(params) => {
                assert_eq!(params.len(), 3);
                assert_eq!(params.get("min").unwrap(), &json!(1));
                assert_eq!(params.get("max").unwrap(), &json!(10));
                assert_eq!(params.get("name").unwrap(), &json!("test"));
            }
            _ => panic!("Expected KeyValue parameters"),
        }
    }

    #[test]
    fn test_directive_with_nested_directives() {
        let directive =
            parse_single_directive("@combine(@length(min=5), @pattern(regex=\"[a-z]+\"))").unwrap();
        assert_eq!(directive.name, "combine");
        match directive.params {
            DirectiveParams::Directives(directives) => {
                assert_eq!(directives.len(), 2);

                let first = &directives[0];
                assert_eq!(first.name, "length");
                match &first.params {
                    DirectiveParams::KeyValue(params) => {
                        assert_eq!(params.get("min").unwrap(), &json!(5));
                    }
                    _ => panic!("Expected KeyValue parameters for length directive"),
                }

                let second = &directives[1];
                assert_eq!(second.name, "pattern");
                match &second.params {
                    DirectiveParams::KeyValue(params) => {
                        assert_eq!(params.get("regex").unwrap(), &json!("[a-z]+"));
                    }
                    _ => panic!("Expected KeyValue parameters for pattern directive"),
                }
            }
            _ => panic!("Expected Directives parameters"),
        }
    }

    #[test]
    fn test_directive_with_empty_array() {
        let directive = parse_single_directive("@values([])").unwrap();
        assert_eq!(directive.name, "values");
        match directive.params {
            DirectiveParams::Array(values) => {
                assert_eq!(values.len(), 0);
            }
            _ => panic!("Expected Array parameters"),
        }
    }

    #[test]
    fn test_directive_with_empty_key_value() {
        let directive = parse_single_directive("@validate()").unwrap();
        assert_eq!(directive.name, "validate");
        match directive.params {
            DirectiveParams::KeyValue(params) => {
                assert!(params.is_empty());
            }
            _ => panic!("Expected KeyValue parameters"),
        }
    }

    #[test]
    fn test_invalid_directives() {
        // assert!(parse_single_directive("validate").is_none()); // Missing @
        assert!(parse_single_directive("@").is_none()); // Missing name
        assert!(parse_single_directive("@validate(").is_none()); // Unclosed parenthesis
        assert!(parse_single_directive("@validate(min=)").is_none()); // Missing value
        assert!(parse_single_directive("@validate(=5)").is_none()); // Missing key
    }

    #[test]
    fn test_complex_nested_directives() {
        let directive = parse_single_directive(
            r#"@group(
                @validate(min=1, max=10),
                @format([1, 2, 3]),
                @nested(@check(value=true))
            )"#,
        )
        .unwrap();

        assert_eq!(directive.name, "group");
        match directive.params {
            DirectiveParams::Directives(directives) => {
                assert_eq!(directives.len(), 3);

                // First nested directive
                let validate = &directives[0];
                assert_eq!(validate.name, "validate");
                match &validate.params {
                    DirectiveParams::KeyValue(params) => {
                        assert_eq!(params.get("min").unwrap(), &json!(1));
                        assert_eq!(params.get("max").unwrap(), &json!(10));
                    }
                    _ => panic!("Expected KeyValue parameters for validate"),
                }

                // Second nested directive
                let format = &directives[1];
                assert_eq!(format.name, "format");
                match &format.params {
                    DirectiveParams::Array(values) => {
                        assert_eq!(values.len(), 3);
                        assert_eq!(values[0], json!(1));
                        assert_eq!(values[1], json!(2));
                        assert_eq!(values[2], json!(3));
                    }
                    _ => panic!("Expected Array parameters for format"),
                }

                // Third nested directive with its own nested directive
                let nested = &directives[2];
                assert_eq!(nested.name, "nested");
                match &nested.params {
                    DirectiveParams::Directives(inner) => {
                        assert_eq!(inner.len(), 1);
                        let check = &inner[0];
                        assert_eq!(check.name, "check");
                        match &check.params {
                            DirectiveParams::KeyValue(params) => {
                                assert_eq!(params.get("value").unwrap(), &json!(true));
                            }
                            _ => panic!("Expected KeyValue parameters for check"),
                        }
                    }
                    _ => panic!("Expected Directives parameters for nested"),
                }
            }
            _ => panic!("Expected Directives parameters"),
        }
    }
}
