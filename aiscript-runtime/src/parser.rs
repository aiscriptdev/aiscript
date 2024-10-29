use serde_json::Value;
use std::collections::HashMap;

use crate::ast::*;
use crate::lexer::{Lexer, Token};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Option<Token>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token().map(|r| r.unwrap());

        Parser {
            lexer,
            current_token,
        }
    }

    fn next_token(&mut self) -> Option<Token> {
        let current = self.current_token.take();
        self.current_token = self.lexer.next_token().map(|r| r.unwrap());
        current
    }

    fn expect_token(&mut self, expected: Token) -> Result<(), String> {
        match self.current_token.take() {
            Some(token) if token == expected => {
                self.current_token = self.lexer.next_token().map(|r| r.unwrap());
                Ok(())
            }
            Some(token) => Err(format!("Expected {:?}, got {:?}", expected, token)),
            None => Err("Unexpected end of input".to_string()),
        }
    }

    pub fn parse_route(&mut self) -> Result<Route, String> {
        let top_route = self.current_token == Some(Token::Route);
        let mut path = (String::from("/"), Vec::new());
        if top_route {
            self.expect_token(Token::Route)?;
            path = self.parse_path()?;
            self.expect_token(Token::OpenBrace)?;
        }

        let mut endpoints = Vec::new();
        while self.current_token.is_some() && self.current_token != Some(Token::CloseBrace) {
            endpoints.push(self.parse_endpoint()?);
        }
        if top_route {
            self.expect_token(Token::CloseBrace)?;
        }

        Ok(Route {
            prefix: path.0,
            params: path.1,
            endpoints,
        })
    }

    fn parse_path(&mut self) -> Result<(String, Vec<PathParameter>), String> {
        let mut path_str = String::new();
        let mut params = Vec::new();

        while let Some(token) = &self.current_token {
            match token {
                Token::Slash => {
                    path_str.push('/');
                    self.next_token();
                }
                Token::OpenAngle => {
                    self.next_token();
                    let name = match self.next_token() {
                        Some(Token::Identifier(name)) => name,
                        _ => return Err("Expected identifier in path parameter".to_string()),
                    };
                    self.expect_token(Token::Colon)?;
                    let param_type = match self.next_token() {
                        Some(Token::TypeStr) => "str".to_string(),
                        Some(Token::TypeInt) => "int".to_string(),
                        Some(Token::TypeBool) => "bool".to_string(),
                        _ => return Err("Expected type in path parameter".to_string()),
                    };
                    self.expect_token(Token::CloseAngle)?;

                    path_str.push(':');
                    path_str.push_str(&name);
                    params.push(PathParameter { name, param_type });
                }
                Token::Identifier(segment) => {
                    path_str.push_str(segment);
                    self.next_token();
                }
                Token::OpenBrace | Token::Comma => break,
                _ => return Err(format!("Unexpected token in path: {:?}", token)),
            }
        }

        Ok((path_str, params))
    }
    fn parse_endpoint(&mut self) -> Result<Endpoint, String> {
        let path_specs = self.parse_path_specs()?;

        self.expect_token(Token::OpenBrace)?;

        // Parse structured parts (query and body)
        let mut query = Vec::new();
        let mut body = RequestBody::default();

        while let Some(token) = &self.current_token {
            match token {
                Token::Query => {
                    self.next_token();
                    query = self.parse_fields()?;
                }
                Token::Body => {
                    self.next_token();
                    body.fields = self.parse_fields()?;
                }
                Token::At => {
                    let directive = self.parse_directive()?;
                    match &*directive.name() {
                        "form" => body.kind = BodyKind::Form,
                        "json" => body.kind = BodyKind::Json,
                        name => {
                            return Err(format!(
                                "Invalid directive, only @form or @json are allowed on body block, current: @{name}"
                            ))
                        }
                    }
                    if let Some(token) = &self.current_token {
                        // let next_token = next_token?;
                        if !matches!(token, Token::Body) {
                            return Err(format!(
                                "Only body block support @form or @json directive, current is {token}",
                            ));
                        }
                    }
                }
                _ => break,
            }
        }

        let statements = match self.lexer.read_raw_script(&self.current_token) {
            Ok(script) => script,
            _ => return Err("Expected script content".to_string()),
        };
        // Get the next token ready for the next endpoint
        self.current_token = self.lexer.next_token().map(|r| r.unwrap());
        Ok(Endpoint {
            path_specs,
            return_type: None,
            query,
            body,
            statements,
        })
    }

    fn parse_path_specs(&mut self) -> Result<Vec<PathSpec>, String> {
        let mut path_specs = Vec::new();

        loop {
            let method = match self.current_token {
                Some(Token::Get) => HttpMethod::Get,
                Some(Token::Post) => HttpMethod::Post,
                Some(Token::Put) => HttpMethod::Put,
                Some(Token::Delete) => HttpMethod::Delete,
                _ => {
                    return Err(format!(
                        "Expected HTTP method, found {:?}",
                        &self.current_token
                    ))
                }
            };
            self.next_token();

            let path = self.parse_path()?;
            path_specs.push(PathSpec {
                method,
                path: path.0,
                params: path.1,
            });

            // Check for comma indicating more paths
            if self.current_token == Some(Token::Comma) {
                self.next_token();
                continue;
            }

            // If we see an opening brace, we're done with paths
            if self.current_token == Some(Token::OpenBrace) {
                break;
            }

            return Err("Expected comma or opening brace after path".to_string());
        }

        Ok(path_specs)
    }

    fn parse_fields(&mut self) -> Result<Vec<Field>, String> {
        self.expect_token(Token::OpenBrace)?;

        let mut fields = Vec::new();
        while self.current_token != Some(Token::CloseBrace) {
            // Parse directives
            let mut directives = Vec::new();
            while let Some(Token::At) = self.current_token {
                directives.push(self.parse_directive()?);
            }

            // Parse field
            let name = match self.next_token() {
                Some(Token::Identifier(name)) => name,
                _ => return Err("Expected field name".to_string()),
            };

            self.expect_token(Token::Colon)?;

            let field_type = match self.next_token() {
                Some(Token::TypeStr) => FieldType::Str,
                Some(Token::TypeInt) => FieldType::Number,
                Some(Token::TypeBool) => FieldType::Bool,
                _ => return Err("Expected field type".to_string()),
            };

            let default = if self.current_token == Some(Token::Equal) {
                self.next_token();
                Some(self.parse_value()?)
            } else {
                None
            };

            fields.push(Field {
                name,
                _type: field_type,
                required: default.is_none(),
                default,
                directives,
            });
        }

        self.expect_token(Token::CloseBrace)?;
        Ok(fields)
    }

    fn parse_directive(&mut self) -> Result<Directive, String> {
        self.expect_token(Token::At)?;

        match self.next_token() {
            Some(Token::Identifier(name)) if name == "any" => {
                // Parse @any directive
                self.expect_token(Token::OpenParen)?;
                let mut directives = Vec::new();
                while self.current_token != Some(Token::CloseParen) {
                    directives.push(self.parse_directive()?);
                    if self.current_token == Some(Token::Comma) {
                        self.next_token();
                    }
                }
                self.expect_token(Token::CloseParen)?;
                Ok(Directive::Any(directives))
            }
            Some(Token::Identifier(name)) if name == "not" => {
                // Parse @not directive
                self.expect_token(Token::OpenParen)?;
                let directive = self.parse_directive()?;
                self.expect_token(Token::CloseParen)?;
                Ok(Directive::Not(Box::new(directive)))
            }
            Some(Token::Identifier(name)) if name == "in" => {
                // Parse @in directive
                self.expect_token(Token::OpenParen)?;
                let values = self.parse_array_values()?;
                self.expect_token(Token::CloseParen)?;
                Ok(Directive::In(values))
            }
            Some(Token::Identifier(name)) => {
                // Parse simple directive
                let mut params = HashMap::new();

                if self.current_token == Some(Token::OpenParen) {
                    self.next_token();

                    while self.current_token != Some(Token::CloseParen) {
                        let param_name = match self.next_token() {
                            Some(Token::Identifier(name)) => name,
                            _ => return Err("Expected parameter name".to_string()),
                        };

                        self.expect_token(Token::Equal)?;
                        let value = self.parse_value()?;
                        params.insert(param_name, value);

                        if self.current_token == Some(Token::Comma) {
                            self.next_token();
                        }
                    }

                    self.expect_token(Token::CloseParen)?;
                }

                Ok(Directive::Simple { name, params })
            }
            _ => Err("Expected directive name".to_string()),
        }
    }

    fn parse_array_values(&mut self) -> Result<Vec<Value>, String> {
        self.expect_token(Token::OpenBracket)?;
        let mut values = Vec::new();
        // FIXME: don't allow empty arrays and nested arrays
        // FIXME: don't allow heterogeneous arrays
        while self.current_token != Some(Token::CloseBracket) {
            values.push(self.parse_value()?);
            if self.current_token == Some(Token::Comma) {
                self.next_token();
            }
        }
        self.expect_token(Token::CloseBracket)?;
        Ok(values)
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.next_token() {
            Some(Token::StringLiteral(s)) => Ok(Value::String(s)),
            Some(Token::NumberLiteral(n)) => Ok(Value::Number(serde_json::Number::from(n))),
            Some(Token::BoolLiteral(b)) => Ok(Value::Bool(b)),
            token => Err(format!("Expected value, got {:?}", token)),
        }
    }
}

// Helper function to make parser usage easier
pub fn parse_route(input: &str) -> Result<Route, String> {
    let mut parser = Parser::new(input);
    parser.parse_route()
}

// Add some tests to verify the implementation
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_route() {
        let input = r#"
            route /test/<id:int> {
                get /a {
                    query {
                        name: str = "hello"
                        age: int = 18
                    }
                    body {
                        @length(max=10)
                        a: str
                        b: bool = false
                    }

                    let greeting = "Hello, {name}"
                    if greeting {
                        print greeting
                    }
                    return greeting, 200
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_ok());

        let route = result.unwrap();
        assert_eq!(route.prefix, "/test/:id");
        assert_eq!(route.params.len(), 1);
        assert_eq!(route.params[0].name, "id");
        assert_eq!(route.params[0].param_type, "int");

        let endpoint = &route.endpoints[0];
        assert_eq!(endpoint.path_specs[0].method, HttpMethod::Get);
        assert_eq!(endpoint.path_specs[0].path, "/a");

        // Verify query parameters
        assert_eq!(endpoint.query.len(), 2);
        assert_eq!(endpoint.query[0].name, "name");
        assert_eq!(endpoint.query[1].name, "age");

        // Verify body fields
        assert_eq!(endpoint.body.fields.len(), 2);
        assert_eq!(endpoint.body.fields[0].name, "a");
        assert_eq!(endpoint.body.fields[1].name, "b");

        // Verify script capture
        assert!(endpoint.statements.contains("let greeting"));
        assert!(endpoint.statements.contains("return greeting, 200"));
    }

    #[test]
    fn test_no_top_route() {
        let input = r#"
            get / {
                return "hello", 200
            }

            post / {
                return "hello", 200
            }
        "#;
        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        // assert!(result.is_ok());
        let route = result.unwrap();
        assert_eq!(route.endpoints.len(), 2);
        assert_eq!(route.endpoints[0].path_specs[0].method, HttpMethod::Get);
        assert_eq!(route.endpoints[1].path_specs[0].method, HttpMethod::Post);
        assert_eq!(route.endpoints[0].path_specs[0].path, "/");
        assert_eq!(route.endpoints[1].path_specs[0].path, "/");
    }

    #[test]
    fn test_multiple_methods() {
        let input = r#"
            route /api {
                get /, post / {
                    return "hello", 200
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        // assert!(result.is_ok());

        let route = result.unwrap();
        let endpoint = &route.endpoints[0];
        assert_eq!(endpoint.path_specs.len(), 2);
        assert_eq!(endpoint.path_specs[0].method, HttpMethod::Get);
        assert_eq!(endpoint.path_specs[1].method, HttpMethod::Post);
    }

    #[test]
    fn test_directives() {
        let input = r#"
            route / {
                post / {
                    @json
                    body {
                        @length(max=10)
                        @not(@another)
                        field: str
                        @in(["a" ,"b", "c"])
                        x: str = "a"
                        @in([1, 2, 3])
                        y: int = 1
                        @any(@a, @b(arg=1), @c)
                        z: str
                    }
                    return "ok", 200
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        // assert!(result.is_ok());

        let route = result.unwrap();
        let endpoint = &route.endpoints[0];

        let field = &endpoint.body.fields[0];
        assert_eq!(field.name, "field");
        assert_eq!(field.directives.len(), 2);
        assert_eq!(
            field.directives[0],
            Directive::Simple {
                name: String::from("length"),
                params: [(String::from("max"), Value::from(10)).into()]
                    .into_iter()
                    .collect::<HashMap<String, Value>>()
            }
        );
        assert_eq!(
            field.directives[1],
            Directive::Not(Box::new(Directive::Simple {
                name: String::from("another"),
                params: HashMap::new(),
            }))
        );

        let field = &endpoint.body.fields[1];
        assert_eq!(field.name, "x");
        assert_eq!(field.default, Some(Value::from("a")));
        assert_eq!(field.directives.len(), 1);
        assert_eq!(
            field.directives[0],
            Directive::In(vec![Value::from("a"), Value::from("b"), Value::from("c"),])
        );

        let field = &endpoint.body.fields[2];
        assert_eq!(field.name, "y");
        assert_eq!(field.default, Some(Value::from(1)));
        assert_eq!(field.directives.len(), 1);
        assert_eq!(
            field.directives[0],
            Directive::In(vec![Value::from(1), Value::from(2), Value::from(3),])
        );

        let field = &endpoint.body.fields[3];
        assert_eq!(field.name, "z");
        assert_eq!(field.directives.len(), 1);
        assert_eq!(
            field.directives[0],
            Directive::Any(vec![
                Directive::Simple {
                    name: String::from("a"),
                    params: HashMap::new(),
                },
                Directive::Simple {
                    name: String::from("b"),
                    params: [(String::from("arg"), Value::from(1)).into()]
                        .into_iter()
                        .collect::<HashMap<String, Value>>(),
                },
                Directive::Simple {
                    name: String::from("c"),
                    params: HashMap::new(),
                },
            ])
        );
    }

    #[test]
    fn test_multiple_methods_single_path() {
        let input = r#"
            route /api {
                get /, post / {
                    return "hello", 200
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route().unwrap();

        assert_eq!(result.prefix, "/api");
        assert_eq!(result.endpoints.len(), 1);

        let endpoint = &result.endpoints[0];
        assert_eq!(endpoint.path_specs.len(), 2);
        assert_eq!(endpoint.path_specs[0].method, HttpMethod::Get);
        assert_eq!(endpoint.path_specs[0].path, "/");
        assert_eq!(endpoint.path_specs[1].method, HttpMethod::Post);
        assert_eq!(endpoint.path_specs[1].path, "/");
    }

    #[test]
    fn test_multiple_paths_with_params() {
        let input = r#"
            route /api {
                get /users/<id:int>, post /users {
                    return "ok", 200
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route().unwrap();

        let endpoint = &result.endpoints[0];
        assert_eq!(endpoint.path_specs.len(), 2);

        assert_eq!(endpoint.path_specs[0].method, HttpMethod::Get);
        assert_eq!(endpoint.path_specs[0].path, "/users/:id");
        assert_eq!(endpoint.path_specs[0].params.len(), 1);
        assert_eq!(endpoint.path_specs[0].params[0].name, "id");

        assert_eq!(endpoint.path_specs[1].method, HttpMethod::Post);
        assert_eq!(endpoint.path_specs[1].path, "/users");
        assert_eq!(endpoint.path_specs[1].params.len(), 0);
    }

    #[test]
    fn test_empty_paths() {
        let input = r#"
            route / {
                get /, post /, put / {
                    return "root", 200
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route().unwrap();

        let endpoint = &result.endpoints[0];
        assert_eq!(endpoint.path_specs.len(), 3);
        assert_eq!(endpoint.path_specs[0].path, "/");
        assert_eq!(endpoint.path_specs[1].path, "/");
        assert_eq!(endpoint.path_specs[2].path, "/");
    }
}
