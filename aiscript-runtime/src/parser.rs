use aiscript_directive::DirectiveParser;
use aiscript_directive::route::RouteAnnotation;
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

impl DerefMut for Parser<'_> {
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
        let annotation = self.parse_route_annotation();
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
            annotation,
            prefix: path.0,
            params: path.1,
            endpoints,
            docs,
        })
    }

    fn parse_endpoint(&mut self) -> Result<Endpoint, String> {
        let annotation = self.parse_route_annotation();
        let path_specs = self.parse_path_specs()?;

        self.consume(TokenType::OpenBrace, "Expect '{' before endpoint")?;

        // Parse docs
        let docs = self.parse_docs();

        // Parse structured parts (query and body)
        let mut path = Vec::new();
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
            } else if self.scanner.check_identifier("path") {
                self.advance();
                path = self.parse_fields()?;
            } else if self.scanner.check(TokenType::At) {
                let directives = DirectiveParser::new(&mut self.scanner).parse_directives();
                for directive in directives {
                    match directive.name.as_str() {
                        "form" => body.kind = BodyKind::Form,
                        "json" => body.kind = BodyKind::Json,
                        name => {
                            return Err(format!(
                                "Invalid directive, only @form or @json are allowed on body block, current: @{name}"
                            ));
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
        let statements = format!(
            "ai fn handler(path, query, body, request, header){{{}}}",
            script
        );
        self.consume(TokenType::CloseBrace, "Expect '}' after endpoint")?;

        let endpoint = Endpoint {
            annotation,
            path_specs,
            return_type: None,
            path,
            query,
            body,
            statements,
            docs,
        };
        // Validate path parameters
        self.validate_path_params(&endpoint)?;
        Ok(endpoint)
    }

    fn parse_route_annotation(&mut self) -> RouteAnnotation {
        let mut annotation = RouteAnnotation::default();
        let directives = DirectiveParser::new(&mut self.scanner).parse_directives();
        for directive in directives {
            let line = directive.line;
            if let Err(error) = annotation.parse_directive(directive) {
                self.error_with_line(line, &error);
            }
        }
        annotation
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
                validators: validators.into_boxed_slice(),
                docs,
            });

            // If this is not the last field (not followed by a closing brace),
            // then a comma is required
            if !self.check(TokenType::CloseBrace) {
                self.consume(TokenType::Comma, "Expected ',' after field definition")?;
            } else {
                // We've reached the closing brace, we can optionally have a comma
                if self.check(TokenType::Comma) {
                    self.advance(); // consume the optional trailing comma
                }
                break;
            }
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

    fn validate_path_params(&self, endpoint: &Endpoint) -> Result<(), String> {
        // Check if any path spec contains parameters
        let has_path_params = endpoint
            .path_specs
            .iter()
            .any(|spec| !spec.params.is_empty());

        // If there are path parameters but no path block, that's an error
        if has_path_params && endpoint.path.is_empty() {
            return Err("Path parameters found in URL but no path block defined".to_string());
        }

        // Skip further validation if no path block is defined (and no params exist)
        if endpoint.path.is_empty() {
            return Ok(());
        }

        // For each path spec, validate that path params match
        for path_spec in &endpoint.path_specs {
            // First check for case-insensitive matches to provide better error messages
            let mut missing_params = Vec::new();
            let mut extra_params = Vec::new();
            let mut case_mismatches = Vec::new();

            // Track which path block params correspond to path spec params
            let mut matched_path_params = std::collections::HashSet::new();

            // Check each path spec parameter
            for param_name in &path_spec.params {
                // Try to find exact match first
                let exact_match = endpoint.path.iter().find(|f| &f.name == param_name);

                if exact_match.is_some() {
                    matched_path_params.insert(param_name.clone());
                    continue;
                }

                // Try case-insensitive match
                let case_insensitive_match = endpoint
                    .path
                    .iter()
                    .find(|f| f.name.to_lowercase() == param_name.to_lowercase());

                if let Some(field) = case_insensitive_match {
                    case_mismatches.push((param_name.clone(), field.name.clone()));
                    matched_path_params.insert(field.name.clone());
                } else {
                    missing_params.push(param_name.clone());
                }
            }

            // Check for extra parameters in path block
            for field in &endpoint.path {
                if !matched_path_params.contains(&field.name) {
                    // Check if it's a case mismatch before marking as extra
                    let is_case_mismatch = path_spec
                        .params
                        .iter()
                        .any(|p| p.to_lowercase() == field.name.to_lowercase());

                    if !is_case_mismatch {
                        extra_params.push(field.name.clone());
                    }
                }
            }

            // Report case mismatches first (most likely cause of issues)
            if !case_mismatches.is_empty() {
                let mismatch_desc = case_mismatches
                    .iter()
                    .map(|(url, block)| format!("'{}' in URL vs '{}' in path block", url, block))
                    .collect::<Vec<_>>()
                    .join(", ");

                return Err(format!("Path parameter case mismatch: {}", mismatch_desc));
            }

            // Report missing parameters
            if !missing_params.is_empty() {
                return Err(format!(
                    "Missing path parameter(s) in path block: {}",
                    missing_params
                        .iter()
                        .map(|s| format!("'{}'", s))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            // Report extra parameters
            if !extra_params.is_empty() {
                return Err(format!(
                    "Unknown path parameter(s) in path block: {}",
                    extra_params
                        .iter()
                        .map(|s| format!("'{}'", s))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }

        Ok(())
    }

    fn parse_path(&mut self) -> Result<(String, Vec<String>), String> {
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
                TokenType::Colon => {
                    self.advance(); // Consume :

                    // Parse parameter name
                    if !self.check(TokenType::Identifier) {
                        return Err("Expected parameter name after ':'".to_string());
                    }
                    let name = self.current.lexeme.to_string();
                    self.advance();

                    // Add parameter to path in the format Axum expects: {id}
                    path.push('{');
                    path.push_str(&name);
                    path.push('}');

                    // Add parameter name to our list
                    params.push(name);
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
        Validator,
        validator::{AnyValidator, InValidator, NotValidator, StringValidator},
    };

    use super::*;

    #[test]
    fn test_basic_route() {
        let input = r#"
            route /test/:id {
                """
                Test route line1
                Test route line2
                """

                get /a {
                    """Test endpoint"""
                    query {
                        """field name"""
                        name: str = "hello",
                        age: int = 18,
                    }
                    body {
                        """field a"""
                        @length(max=10)
                        a: str,
                        b: bool = false,
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
        assert_eq!(route.prefix, "/test/{id}");
        assert_eq!(route.params.len(), 1);
        assert_eq!(route.params[0], "id");

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
                        field: str,
                        @in(["a" ,"b", "c"])
                        x: str = "a",
                        @in([1, 2, 3])
                        y: int = 1,
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
        assert_eq!(field.validators.len(), 2);
        assert_eq!(field.validators[0].name(), "@string");
        assert_eq!(
            field.validators[0]
                .downcast_ref::<StringValidator>()
                .unwrap()
                .max_len,
            Some(10)
        );
        assert_eq!(
            field.validators[1]
                .downcast_ref::<NotValidator<Box<dyn Validator>>>()
                .unwrap()
                .0
                .downcast_ref::<StringValidator>()
                .unwrap()
                .min_len,
            Some(5)
        );

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
            validators[1]
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
    #[ignore = "temporary ignore"]
    fn test_multiple_paths_with_params() {
        let input = r#"
            route /api {
                get /users/:id, post /users {
                    return "ok";
                }
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route().unwrap();

        let endpoint = &result.endpoints[0];
        assert_eq!(endpoint.path_specs.len(), 2);

        assert_eq!(endpoint.path_specs[0].method, HttpMethod::Get);
        assert_eq!(endpoint.path_specs[0].path, "/users/{id}");
        assert_eq!(endpoint.path_specs[0].params.len(), 1);
        assert_eq!(endpoint.path_specs[0].params[0], "id");

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

    #[test]
    fn test_path_param_validation() {
        // Test 1: Successful case - path parameters in path spec match path block
        let input = r#"
            get /users/:id/posts/:postId {
                path {
                    @string(min_len=3)
                    id: str,
                    postId: int,
                }
                
                return "Valid";
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_ok());
        let route = result.unwrap();
        assert_eq!(route.endpoints.len(), 1);

        // Test 2: Path parameter name mismatch (postid vs postId)
        let input = r#"
            get /users/:id/posts/:postid {
                path {
                    @string(min_len=3)
                    id: str,
                    postId: int,
                }
                
                return "Invalid";
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("Path parameter case mismatch"));
        assert!(error.contains("'postid' in URL vs 'postId' in path block"));

        // Test 3: Extra parameter in path block
        let input = r#"
            get /users/:id/posts/:postId {
                path {
                    @string(min_len=3)
                    id: str,
                    postId: int,
                    name: str
                }
                
                return "Invalid";
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("Unknown path parameter(s)"));
        assert!(error.contains("'name'"));

        // Test 4: Missing parameter in path block
        let input = r#"
            get /users/:id/posts/:postId {
                path {
                    postId: int,
                }
                
                return "Invalid";
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("Missing path parameter(s)"));
        assert!(error.contains("'id'"));

        // Test 5: Case sensitivity check
        let input = r#"
            get /users/:ID/posts/:postId {
                path {
                    id: str,
                    postId: int,
                }
                
                return "Invalid";
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("case mismatch"));

        // // Test 6: Type mismatch
        // let input = r#"
        //     get /users/:id/posts/:postId {
        //         path {
        //             id: int,  // Should be string based on the path parameter type
        //             postId: str, // Should be int based on the path parameter type
        //         }

        //         return "Invalid";
        //     }
        // "#;
        // let mut parser = Parser::new(input);
        // let result = parser.parse_route();
        // assert!(result.is_err());
        // let error = result.unwrap_err();
        // assert!(error.contains("Type mismatch"));

        // Test 7: Multiple path specs with the same parameters
        let input = r#"
            get /users/:id, post /users/:id {
                path {
                    id: str,
                }
                
                return "Valid";
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_ok());

        // Test 8: Multiple path specs with different parameters (should fail)
        let input = r#"
            get /users/:id, post /users/:userId {
                path {
                    id: str,
                }
                
                return "Invalid";
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("parameter(s)"));

        // Test 9: No path block but path params - should be invalid
        let input = r#"
            get /users/:id {
                return "Invalid - missing path block";
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("Path parameters found in URL but no path block defined"));

        // Test 10: No path params and no path block - should be valid
        let input = r#"
            get /users/all {
                return "Valid - no path params";
            }
        "#;

        let mut parser = Parser::new(input);
        let result = parser.parse_route();
        assert!(result.is_ok());
    }
}
