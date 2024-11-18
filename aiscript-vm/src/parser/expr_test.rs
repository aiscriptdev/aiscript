#[cfg(test)]
mod tests {
    use gc_arena::arena::rootless_mutate;

    use crate::{
        ast::{Expr, Literal, Stmt},
        parser::Parser,
        string::InternedStringSet,
        vm::Context,
    };

    fn assert_object_expr(expr: &Expr, expected_properties: &[(&str, f64)]) {
        if let Expr::Object { properties, .. } = expr {
            assert_eq!(
                properties.len(),
                expected_properties.len(),
                "Wrong number of properties"
            );

            for ((token, value), (expected_name, expected_value)) in
                properties.iter().zip(expected_properties)
            {
                assert_eq!(token.lexeme, *expected_name, "Wrong property name");

                if let Expr::Literal {
                    value: Literal::Number(n),
                    ..
                } = value
                {
                    assert_eq!(*n, *expected_value, "Wrong property value");
                } else {
                    panic!("Expected number literal for property value");
                }
            }
        } else {
            panic!("Expected object expression");
        }
    }

    #[test]
    fn test_object_literals() {
        rootless_mutate(|mutation| {
            let context = Context {
                mutation,
                strings: InternedStringSet::new(mutation),
            };

            // empty object
            let mut parser = Parser::new(context, "let x = {};");
            let program = parser.parse().unwrap();
            match &program.statements[0] {
                Stmt::Let(var_decl) => {
                    if let Some(Expr::Object { properties, .. }) = &var_decl.initializer {
                        assert!(properties.is_empty(), "Object should have no properties");
                    } else {
                        panic!("Expected empty object expression");
                    }
                }
                _ => panic!("Expected let statement"),
            }

            // single property object
            let mut parser = Parser::new(context, "let x = {a: 1};");
            let program = parser.parse().unwrap();
            match &program.statements[0] {
                Stmt::Let(var_decl) => {
                    if let Some(expr) = &var_decl.initializer {
                        assert_object_expr(expr, &[("a", 1.0)]);
                    }
                }
                _ => panic!("Expected let statement"),
            }

            // multiple property object
            let mut parser = Parser::new(context, "let x = {a: 1, b: 2, c: 3};");
            let program = parser.parse().unwrap();
            match &program.statements[0] {
                Stmt::Let(var_decl) => {
                    if let Some(expr) = &var_decl.initializer {
                        assert_object_expr(expr, &[("a", 1.0), ("b", 2.0), ("c", 3.0)]);
                    }
                }
                _ => panic!("Expected let statement"),
            }

            // nested object literal
            let mut parser = Parser::new(context, "let x = {a: 1, b: {c: 2}};");
            let program = parser.parse().unwrap();
            match &program.statements[0] {
                Stmt::Let(var_decl) => {
                    if let Some(Expr::Object { properties, .. }) = &var_decl.initializer {
                        assert_eq!(properties.len(), 2, "Should have 2 top-level properties");

                        // Check first property
                        let (first_key, first_value) = &properties[0];
                        assert_eq!(first_key.lexeme, "a");
                        assert!(matches!(
                            first_value,
                            Expr::Literal {
                                value: Literal::Number(1.0),
                                ..
                            }
                        ));

                        // Check nested object
                        let (second_key, second_value) = &properties[1];
                        assert_eq!(second_key.lexeme, "b");
                        assert_object_expr(second_value, &[("c", 2.0)]);
                    }
                }
                _ => panic!("Expected let statement"),
            }

            // trailing comma
            let mut parser = Parser::new(context, "let x = {a: 1, b: 2,};");
            let program = parser.parse().unwrap();
            match &program.statements[0] {
                Stmt::Let(var_decl) => {
                    if let Some(expr) = &var_decl.initializer {
                        assert_object_expr(expr, &[("a", 1.0), ("b", 2.0)]);
                    }
                }
                _ => panic!("Expected let statement"),
            }
        });
    }
}
