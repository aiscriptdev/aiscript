use aiscript_directive::{Directive, DirectiveParser};
use serde_json::Value;
use std::ops::{Deref, DerefMut};

use crate::ast::*;
use crate::lexer::{Scanner, TokenType};

pub struct Parser<'a> {
    scanner: Scanner<'a>,
}

impl<'a> Deref for Parser<'a> {
    type Target = Scanner<'a>;
    fn deref(&self) -> &Self::Target {
        &self.scanner
    }
}

impl<'a> DerefMut for Parser<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.scanner
    }
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut scanner = Scanner::new(source);
        scanner.advance();
        Parser { scanner }
    }

    fn parse_docs(&mut self) -> String {
        let mut docs = String::new();
        if self.match_token(TokenType::Doc) {
            let lines = self
                .previous
                .lexeme
                .lines()
                .map(|line| line.trim())
                .collect::<Vec<_>>();
            docs = lines.join("\n");
        }
        docs
    }

    pub fn parse_route(&mut self) -> Result<Route, String> {
        let directives = DirectiveParser::new(&mut self.scanner).parse_directives();
        let mut docs = String::new();
        let mut path = (String::from("/"), Vec::new());

        // Check if this is a top-level route declaration
        let is_top_route = self.check_identifier("route");
        if is_top_route {
            self.advance(); // consume 'route'
            path = self.parse_path()?;
            self.consume(TokenType::OpenBrace, "Expect '{' after route path")?;

            docs = self.parse_docs();
        }

        let mut endpoints = Vec::new();
        while !self.is_at_end() && !self.check(TokenType::CloseBrace) {
            endpoints.push(self.parse_endpoint()?);
        }

        if is_top_route {
            self.consume(TokenType::CloseBrace, "Expect '}' after route body")?;
        }

        Ok(Route {
            auth: self.parse_auth(&directives),
            prefix: path.0,
            params: path.1,
            endpoints,
            docs,
        })
    }

    fn parse_endpoint(&mut self) -> Result<Endpoint, String> {
        let directives = DirectiveParser::new(&mut self.scanner).parse_directives();
        let path_specs = self.parse_path_specs()?;

        self.consume(TokenType::OpenBrace, "Expect '{' before endpoint")?;

        // Parse docs
        let docs = self.parse_docs();

        // Parse structured parts (query and body)
        let mut query = Vec::new();
        let mut body = RequestBody::default();

        // Only parse structured blocks (query/body) and directives
        while !self.is_at_end() {
            if self.scanner.check_identifier("query") {
                self.advance();
                query = self.parse_fields()?;
            } else if self.scanner.check_identifier("body") {
                self.advance();
                body.fields = self.parse_fields()?;
            } else if self.scanner.check(TokenType::At) {
                let directives = DirectiveParser::new(&mut self.scanner).parse_directives();
                for directive in directives {
                    match directive.name.as_str() {
                            "form" => body.kind = BodyKind::Form,
                            "json" => body.kind = BodyKind::Json,
                            name => {
                                return Err(format!(
                                    "Invalid directive, only @form or @json are allowed on body block, current: @{name}"
                                ))
                            }
                        }

                    if !self.check_identifier("body") {
                        return Err("Only body block supports @form or @json directive".into());
                    }
                }
            } else {
                // Break for anything else to handle raw script
                break;
            }
        }

        if self.check(TokenType::CloseBrace) {
            return Err("Route without handler script is not allowed.".to_string());
        }
        // Parse the handler function body
        let script = self.read_raw_script()?;
        let statements = format!("ai fn handler(query, body, request, header){{{}}}", script);
        self.consume(TokenType::CloseBrace, "Expect '}' after endpoint")?;
        Ok(Endpoint {
            auth: self.parse_auth(&directives),
            path_specs,
            return_type: None,
            query,
            body,
            statements,
            docs,
        })
    }

    fn parse_auth(&mut self, directives: &[Directive]) -> Auth {
        directives
            .iter()
            .rfind(|d| ["auth", "basic_auth"].contains(&d.name.as_str()))
            .map_or(Auth::None, |d| match d.name.as_str() {
                "auth" => Auth::Jwt,
                "basic_auth" => Auth::Basic,
                _ => Auth::None,
            })
    }

    fn parse_fields(&mut self) -> Result<Vec<Field>, String> {
        self.consume(TokenType::OpenBrace, "Expected '{' after field block")?;

        let mut fields = Vec::new();
        while !self.check(TokenType::CloseBrace) {
            // Parse doc comments
            let docs = self.parse_docs();

            // Parse validators
            let validators = DirectiveParser::new(&mut self.scanner).parse_validators();

            // Parse field name
            if !self.check(TokenType::Identifier) {
                return Err("Expected field name".to_string());
            }
            let name = self.current.lexeme.to_string();
            self.advance();

            self.consume(TokenType::Colon, "Expected ':' after field name")?;

            // Parse field type
            if !self.check(TokenType::Identifier) {
                return Err("Expected field type".to_string());
            }
            let field_type = match self.current.lexeme {
                "str" => FieldType::Str,
                "int" | "float" => FieldType::Number,
                "bool" => FieldType::Bool,
                _ => return Err(format!("Invalid field type: {}", self.current.lexeme)),
            };
            self.advance();

            // Parse default value
            let mut default = None;
            if self.check(TokenType::Equal) {
                self.advance();
                default = Some(self.parse_value()?);
            }

            fields.push(Field {
                name,
                _type: field_type,
                required: default.is_none(),
                default,
                validators,
                docs,
            });
        }

        self.consume(TokenType::CloseBrace, "Expected '}' after fields")?;
        Ok(fields)
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        let value = match self.current.kind {
            TokenType::Number => {
                if self.current.lexeme.contains(".") {
                    let num = self
                        .current
                        .lexeme
                        .parse::<f64>()
                        .map_err(|_| "Invalid number".to_string())?;
                    Value::Number(serde_json::Number::from_f64(num).ok_or("Invalid number")?)
                } else {
                    let num = self
                        .current
                        .lexeme
                        .parse::<i64>()
                        .map_err(|_| "Invalid number".to_string())?;
                    Value::Number(serde_json::Number::from(num))
                }
            }
            TokenType::String => {
                let lexeme = self.current.lexeme;
                let escaped_string = self
                    .escape_string(lexeme)
                    .ok_or_else(|| String::from("Invalid string"))?;
                Value::String(escaped_string)
            }
            TokenType::True => Value::Bool(true),
            TokenType::False => Value::Bool(false),
            _ => return Err("Expected value".to_string()),
        };
        self.advance();
        Ok(value)
    }

    fn parse_path_specs(&mut self) -> Result<Vec<PathSpec>, String> {
        let mut specs = Vec::new();

        loop {
            // Parse HTTP method
            if !self.check(TokenType::Identifier) {
                return Err("Expected HTTP method".to_string());
            }

            let method = match self.current.lexeme {
                "get" => HttpMethod::Get,
                "post" => HttpMethod::Post,
                "put" => HttpMethod::Put,
                "delete" => HttpMethod::Delete,
                _ => return Err(format!("Invalid HTTP method: {}", self.current.lexeme)),
            };
            self.advance();

            // Parse path
            let (path, params) = self.parse_path()?;

            specs.push(PathSpec {
                method,
                path,
                params,
            });

            // Check for more paths
            if self.check(TokenType::Comma) {
                self.advance();
                continue;
            }
            break;
        }

        Ok(specs)
    }

    fn parse_path(&mut self) -> Result<(String, Vec<PathParameter>), String> {
        let mut path = String::new();
        let mut params = Vec::new();

        // Handle leading slash
        if self.check(TokenType::Slash) {
            path.push('/');
            self.advance();
        }

        while !self.is_at_end() {
            match self.current.kind {
                TokenType::Slash => {
                    path.push('/');
                    self.advance();
                }
                TokenType::Less => {
                    self.advance(); // Consume <

                    // Parse parameter name
                    if !self.check(TokenType::Identifier) {
                        return Err("Expected parameter name".to_string());
                    }
                    let name = self.current.lexeme.to_string();
                    self.advance();

                    self.consume(TokenType::Colon, "Expected ':' after parameter name")?;

                    // Parse parameter type
                    if !self.check(TokenType::Identifier) {
                        return Err("Expected parameter type".to_string());
                    }
                    let param_type = self.current.lexeme.to_string();
                    self.advance();

                    self.consume(TokenType::Greater, "Expected '>' after parameter type")?;

                    path.push(':');
                    path.push_str(&name);
                    params.push(PathParameter { name, param_type });
                }
                TokenType::Identifier => {
                    path.push_str(self.current.lexeme);
                    self.advance();
                }
                TokenType::OpenBrace | TokenType::Comma => break,
                _ => return Err(format!("Unexpected token in path: {:?}", self.current.kind)),
            }
        }

        Ok((path, params))
    }

    fn consume(&mut self, expected: TokenType, message: &str) -> Result<(), String> {
        if self.check(expected) {
            self.advance();
            Ok(())
        } else {
            Err(message.to_string())
        }
    }
}

pub fn parse_route(input: &str) -> Result<Route, String> {
    let mut parser = Parser::new(input);
    parser.parse_route()
}

#[cfg(test)]
mod tests {
    use aiscript_directive::{
        validator::{AnyValidator, InValidator, NotValidator, StringValidator},
        Validator,
    };

    use super::*;

    #[test]
    fn test_basic_route() {
        let input = r#"
            route /test/<id:int> {
                """
                Test route line1
                Test route line2
                """

                get /a {
                    """Test endpoint"""
                    query {
                        """field name"""
                        name: str = "hello"
                        age: int = 18
                    }
                    body {
                        """field a"""
                        @length(max=10)
                        a: str
                        b: bool = false
                    }

                    let greeting = "Hello" + name;
                    if greeting {
                        print(greeting);
                    }
                    return greeting;
                }

                post /b {
                    """Test endpoint2"""

                    return "endpoint2";
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();

        let route = result.unwrap();
        assert_eq!(route.docs, "Test route line1\nTest route line2");
        assert_eq!(route.prefix, "/test/:id");
        assert_eq!(route.params.len(), 1);
        assert_eq!(route.params[0].name, "id");
        assert_eq!(route.params[0].param_type, "int");

        let endpoint = &route.endpoints[0];
        assert_eq!(endpoint.docs, "Test endpoint");
        assert_eq!(endpoint.path_specs[0].method, HttpMethod::Get);
        assert_eq!(endpoint.path_specs[0].path, "/a");

        // Verify query parameters
        assert_eq!(endpoint.query.len(), 2);
        assert_eq!(endpoint.query[0].docs, "field name");
        assert_eq!(endpoint.query[0].name, "name");
        assert_eq!(endpoint.query[1].name, "age");

        // Verify body fields
        assert_eq!(endpoint.body.fields.len(), 2);
        assert_eq!(endpoint.body.fields[0].docs, "field a");
        assert_eq!(endpoint.body.fields[0].name, "a");
        assert_eq!(endpoint.body.fields[1].name, "b");

        // Verify script capture
        assert!(endpoint.statements.contains("let greeting"));
        assert!(endpoint.statements.contains("return greeting"));

        // Verify endpoint2
        let endpoint2 = &route.endpoints[1];
        assert_eq!(endpoint2.docs, "Test endpoint2");
        assert_eq!(endpoint2.path_specs[0].method, HttpMethod::Post);
        assert_eq!(endpoint2.path_specs[0].path, "/b");
        assert!(endpoint2.statements.contains("return \"endpoint2\""));
    }

    #[test]
    fn test_no_top_route() {
        let input = r#"
            get / {
                return "hello";
            }

            post / {
                return "hello";
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
                    return "hello";
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
    fn test_validators() {
        let input = r#"
            route / {
                post / {
                    @json
                    body {
                        @string(max_len=10)
                        @not(@string(min_len=5))
                        field: str
                        @in(["a" ,"b", "c"])
                        x: str = "a"
                        @in([1, 2, 3])
                        y: int = 1
                        @any(@in(["a", "b"]), @string(min_len=1))
                        z: str
                    }

                    return "hi";
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_ok());

        let route = result.unwrap();
        let endpoint = &route.endpoints[0];

        let field = &endpoint.body.fields[0];
        assert_eq!(field.name, "field");
        // assert_eq!(field.validators.len(), 2);
        // assert_eq!(
        //     field.validators[0]
        //         .downcast_ref::<StringValidator>()
        //         .unwrap()
        //         .max_len,
        //     Some(10)
        // );
        // assert_eq!(
        //     field.validators[1]
        //         .downcast_ref::<NotValidator<Box<dyn Validator>>>()
        //         .unwrap()
        //         .0
        //         .downcast_ref::<StringValidator>()
        //         .unwrap()
        //         .min_len,
        //     Some(5)
        // );

        let field = &endpoint.body.fields[1];
        assert_eq!(field.name, "x");
        assert_eq!(field.default, Some(Value::from("a")));
        assert_eq!(field.validators.len(), 1);
        assert_eq!(
            field.validators[0].downcast_ref::<InValidator>().unwrap().0,
            vec![Value::from("a"), Value::from("b"), Value::from("c")]
        );

        let field = &endpoint.body.fields[2];
        assert_eq!(field.name, "y");
        assert_eq!(field.default, Some(Value::from(1)));
        assert_eq!(field.validators.len(), 1);
        assert_eq!(
            field.validators[0].downcast_ref::<InValidator>().unwrap().0,
            vec![Value::from(1), Value::from(2), Value::from(3),]
        );

        let field = &endpoint.body.fields[3];
        assert_eq!(field.name, "z");
        assert_eq!(field.validators.len(), 1);
        let validators = &field.validators[0]
            .downcast_ref::<AnyValidator<Box<dyn Validator>>>()
            .unwrap()
            .0;
        assert_eq!(
            validators[0].downcast_ref::<InValidator>().unwrap().0,
            vec![Value::from("a"), Value::from("b")]
        );
        assert_eq!(
            field.validators[0]
                .downcast_ref::<StringValidator>()
                .unwrap()
                .min_len,
            Some(1)
        );
    }

    #[test]
    fn test_multiple_methods_single_path() {
        let input = r#"
            route /api {
                get /, post / {
                    return "hello";
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
                    return "ok";
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
                    return "root";
                }

                get /hi {
                    return "hi;
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route().unwrap();

        assert_eq!(result.endpoints.len(), 2);
        let endpoint = &result.endpoints[0];
        assert_eq!(endpoint.path_specs.len(), 3);
        assert_eq!(endpoint.path_specs[0].path, "/");
        assert_eq!(endpoint.path_specs[1].path, "/");
        assert_eq!(endpoint.path_specs[2].path, "/");
    }
}
