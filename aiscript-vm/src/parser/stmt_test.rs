#[cfg(test)]
mod tests {
    use gc_arena::arena::rootless_mutate;

    use crate::{
        ast::{AgentDecl, FunctionDecl, Stmt},
        parser::Parser,
        string::InternedStringSet,
        vm::Context,
    };

    #[test]
    fn test_parse_function() {
        rootless_mutate(|mutation| {
            let context = Context {
                mutation,
                strings: InternedStringSet::new(mutation),
            };
            let source = r#"
                fn test1() {
                    """
                    test1 doc
                    """
                    print("Hello, world!");
                }

                fn test2(a: int, b: int) -> int {
                    return a + b;
                }

                fn test3(a, b,) -> int {
                    return a + b;
                }
            "#;
            let mut parser = Parser::new(context, &source);
            let result = parser.parse().unwrap();
            let Stmt::Function(FunctionDecl {
                name,
                mangled_name,
                doc,
                params,
                return_type,
                body,
                line,
                ..
            }) = &result.statements[0]
            else {
                panic!("Unexpected statement type");
            };
            assert_eq!(name.lexeme, "test1");
            assert_eq!(mangled_name, "script$test1");
            assert_eq!(doc.unwrap().lexeme, "test1 doc");
            assert_eq!(params.len(), 0);
            assert!(return_type.is_none());
            assert_eq!(body.len(), 1);
            assert_eq!(line, &2);

            let Stmt::Function(FunctionDecl {
                name,
                mangled_name,
                doc,
                params,
                return_type,
                ..
            }) = &result.statements[1]
            else {
                panic!("Unexpected statement type");
            };
            assert_eq!(name.lexeme, "test2");
            assert_eq!(mangled_name, "script$test2");
            assert_eq!(doc, &None);
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].type_hint.unwrap().lexeme, "int");
            assert_eq!(params[1].type_hint.unwrap().lexeme, "int");
            assert_eq!(return_type.unwrap().lexeme, "int");

            let Stmt::Function(FunctionDecl {
                name,
                mangled_name,
                doc,
                params,
                return_type,
                ..
            }) = &result.statements[2]
            else {
                panic!("Unexpected statement type");
            };
            assert_eq!(name.lexeme, "test3");
            assert_eq!(mangled_name, "script$test3");
            assert_eq!(doc, &None);
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].type_hint, None);
            assert_eq!(params[1].type_hint, None);
            assert_eq!(return_type.unwrap().lexeme, "int");
        });
    }

    #[test]
    fn test_parse_agent() {
        rootless_mutate(|mutation| {
            let context = Context {
                mutation,
                strings: InternedStringSet::new(mutation),
            };
            let source = r#"
                agent Test {
                    instructions: "Test instruction.",
                    model: "gpt-4",
                    tools: [a, b],
                    tool_choice: "auto",
                }
            "#;
            let mut parser = Parser::new(context, &source);
            let result = parser.parse().unwrap();
            let Stmt::Agent(AgentDecl {
                name, fields, line, ..
            }) = &result.statements[0]
            else {
                panic!("Expected agent statement");
            };
            assert_eq!(name.lexeme, "Test");
            assert_eq!(*line, 2);
            assert_eq!(fields.len(), 4);
            // let pairs = fields
            //     .iter()
            //     .map(|(key, value)| (key.to_string(), value))
            //     .collect::<Vec<_>>();
            // assert_eq!(vec![("instructions", Expr::Literal(StringLiteral "Test instruction.")),], pairs);

            let source = r#"
                agent Test {
                    model: "gpt-4",
                    tools: [a, b],
                    tool_choice: "auto",
                }
            "#;
            let mut parser = Parser::new(context, &source);
            let result = parser.parse();
            assert!(result.is_err());
            let source = r#"
                agent Test {
                    instructions: "Test instruction.",
                }
            "#;
            let mut parser = Parser::new(context, &source);
            let result = parser.parse().unwrap();
            let Stmt::Agent(AgentDecl { name, fields, .. }) = &result.statements[0] else {
                panic!("Expected agent statement");
            };
            assert_eq!(name.lexeme, "Test");
            assert_eq!(fields.len(), 1);

            let source = r#"
                agent Test {
                    instructions: "Test instruction.",
                    tools: [a, b],
                    tools: [a, b],
                    tool_choice: "auto",
                }
            "#;
            let mut parser = Parser::new(context, &source);
            let result = parser.parse();
            assert!(result.is_err());

            let source = r#"
                agent Test {
                    instructions: "Test instruction.",
                    tools: [a, b],
                    invalid: "invalid field",
                    tool_choice: "auto",
                }
            "#;
            let mut parser = Parser::new(context, &source);
            let result = parser.parse();
            assert!(result.is_err());

            let source = r#"
                agent Test {
                    instructions: "Test instruction.",
                    tools: "invalid tools",
                    tool_choice: "auto",
                }
            "#;
            let mut parser = Parser::new(context, &source);
            let result = parser.parse();
            assert!(result.is_err());
        });
    }
}
