#[cfg(test)]
mod tests {
    use crate::{Lexer, TokenType};

    #[test]
    fn test_basic_fstring() {
        let source = r#"f"Hello, World!""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(token.lexeme, r#"Hello, World!"#);
    }

    #[test]
    fn test_fstring_with_interpolation() {
        let source = r#"f"Hello, {name}!""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(token.lexeme, r#"Hello, {name}!"#);
    }

    #[test]
    fn test_fstring_with_multiple_interpolations() {
        let source = r#"f"{greeting}, {name}! Today is {day}.""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(token.lexeme, r#"{greeting}, {name}! Today is {day}."#);
    }

    #[test]
    fn test_fstring_with_nested_braces() {
        let source = r#"f"Nested: {{not interpolated}} but {interpolated}""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(
            token.lexeme,
            r#"Nested: {{not interpolated}} but {interpolated}"#
        );
    }

    #[test]
    fn test_fstring_with_escaped_quotes() {
        let source = r#"f"This has \"escaped quotes\" and {variable}""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(
            token.lexeme,
            r#"This has \"escaped quotes\" and {variable}"#
        );
    }

    #[test]
    fn test_fstring_with_expressions() {
        let source = r#"f"Result: {x + y * z}""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(token.lexeme, r#"Result: {x + y * z}"#);
    }

    #[test]
    fn test_empty_fstring() {
        let source = r#"f"""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(token.lexeme, r#""#);
    }

    #[test]
    fn test_fstring_with_unicode() {
        let source = r#"f"Hello 你好 {name} 世界!""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(token.lexeme, r#"Hello 你好 {name} 世界!"#);
    }

    #[test]
    fn test_unterminated_fstring() {
        let source = r#"f"Unterminated"#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::Invalid);
        assert!(token.lexeme.contains("Unterminated"));
    }

    #[test]
    fn test_fstring_followed_by_other_tokens() {
        let source = r#"f"Hello, {name}!" + variable"#;
        let mut scanner = Lexer::new(source);

        let token1 = scanner.next().unwrap();
        assert_eq!(token1.kind, TokenType::FString);
        assert_eq!(token1.lexeme, r#"Hello, {name}!"#);

        let token2 = scanner.next().unwrap();
        assert_eq!(token2.kind, TokenType::Plus);

        let token3 = scanner.next().unwrap();
        assert_eq!(token3.kind, TokenType::Identifier);
        assert_eq!(token3.lexeme, "variable");
    }

    #[test]
    fn test_fstring_with_newlines() {
        let source = "f\"Line 1\nLine 2 {variable}\nLine 3\"";
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(token.lexeme, "Line 1\nLine 2 {variable}\nLine 3");
        // Line number is 3 because the token ends on the third line
        // and the lexer tracks the current line
        assert_eq!(token.line, 3);
    }

    #[test]
    fn test_escaped_braces_in_fstring() {
        let source = r#"f"Escaped braces: \{not interpolated\}""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        assert_eq!(token.lexeme, r#"Escaped braces: \{not interpolated\}"#);
    }

    #[test]
    fn test_fstring_with_complex_nested_expressions() {
        // Using single quotes to avoid issues with nested double quotes
        // In a real scenario, users would need to escape internal quotes
        let source = r#"f"Complex: {fn_call(arg1, {nested: \"object\"})[index].property}""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::FString);
        // The lexer will include the escaped quotes in the lexeme
        assert_eq!(
            token.lexeme,
            r#"Complex: {fn_call(arg1, {nested: \"object\"})[index].property}"#
        );
    }

    #[test]
    fn test_normal_f_identifier() {
        // 'f' followed by something other than a string should be an identifier
        let source = "foo";
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::Identifier);
        assert_eq!(token.lexeme, "foo");
    }

    #[test]
    fn test_fstring_vs_f_identifier() {
        // Test 'f' as standalone identifier vs. start of an f-string
        let source = "f f\"string\"";
        let mut scanner = Lexer::new(source);

        let token1 = scanner.next().unwrap();
        assert_eq!(token1.kind, TokenType::Identifier);
        assert_eq!(token1.lexeme, "f");

        let token2 = scanner.next().unwrap();
        assert_eq!(token2.kind, TokenType::FString);
        assert_eq!(token2.lexeme, "string");
    }
}
