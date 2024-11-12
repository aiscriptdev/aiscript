#[cfg(test)]
mod tests {
    use crate::lexer::{Scanner, Token, TokenType};

    #[test]
    fn test_docstring_basic() {
        let source = r#"fn test1() {
    """test1 doc"""
    print("Hello, world!");
}"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let doc_token = tokens
            .iter()
            .find(|t| t.kind == TokenType::Doc)
            .expect("Docstring token not found");

        assert_eq!(doc_token.lexeme, "test1 doc");
    }

    #[test]
    fn test_docstring_multiline() {
        let source = r#"fn test2() {
    """
    This is a
    multiline docstring
    """
    print("Hello");
}"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let doc_token = tokens
            .iter()
            .find(|t| t.kind == TokenType::Doc)
            .expect("Docstring token not found");

        assert_eq!(doc_token.lexeme, "This is a\n    multiline docstring");
    }

    #[test]
    fn test_docstring_whitespace() {
        let source = r#""""    spaces before and after    """"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let doc_token = tokens
            .iter()
            .find(|t| t.kind == TokenType::Doc)
            .expect("Docstring token not found");

        assert_eq!(doc_token.lexeme, "spaces before and after");
    }

    #[test]
    fn test_docstring_unterminated() {
        let source = r#"fn test() {
    """unterminated docstring
    "#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let error_token = tokens
            .iter()
            .find(|t| t.kind == TokenType::Error)
            .expect("Error token not found");

        assert_eq!(error_token.lexeme, "Unterminated docstring.");
    }

    #[test]
    fn test_empty_docstring() {
        let source = r#"
        """      """
        "#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let doc_token = dbg!(&tokens)
            .iter()
            .find(|t| t.kind == TokenType::Doc)
            .expect("Docstring token not found");

        assert_eq!(doc_token.lexeme, "");

        let source = r#""""""""#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let doc_token = &tokens
            .iter()
            .find(|t| t.kind == TokenType::Doc)
            .expect("Docstring token not found");

        assert_eq!(doc_token.lexeme, "");
    }
    #[test]
    fn test_docstring() {
        let source = r#"fn test() {
    """
    This is a
    multiline docstring
    """
    print("Hello");
}"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let doc_token = tokens
            .iter()
            .find(|t| t.kind == TokenType::Doc)
            .expect("Docstring token not found");

        assert_eq!(doc_token.lexeme, "This is a\n    multiline docstring");
    }

    #[test]
    fn test_unterminated_docstring() {
        let source = r#"fn test() {
    """
    Unterminated docstring
    "#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let error_token = tokens
            .iter()
            .find(|t| t.kind == TokenType::Error)
            .expect("Error token not found");

        assert_eq!(error_token.lexeme, "Unterminated docstring.");
    }

    #[test]
    fn test_string_tokens() {
        let source = r#"print("Hello" "World");"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        // Verify the sequence of tokens
        assert_eq!(tokens[0].kind, TokenType::Print);
        assert_eq!(tokens[1].kind, TokenType::OpenParen);
        assert_eq!(tokens[2].kind, TokenType::String);
        assert_eq!(tokens[3].kind, TokenType::String);
        assert_eq!(tokens[4].kind, TokenType::CloseParen);
        assert_eq!(tokens[5].kind, TokenType::Semicolon);

        // Verify the string contents
        assert_eq!(tokens[2].lexeme, "\"Hello\"");
        assert_eq!(tokens[3].lexeme, "\"World\"");
    }

    #[test]
    fn test_numbers() {
        let source = "123 123.456 0.1";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        // Only check the number tokens, ignoring whitespace
        let number_tokens: Vec<&Token> = tokens
            .iter()
            .filter(|t| t.kind == TokenType::Number)
            .collect();

        assert_eq!(number_tokens.len(), 3);
        assert_eq!(number_tokens[0].lexeme, "123");
        assert_eq!(number_tokens[1].lexeme, "123.456");
        assert_eq!(number_tokens[2].lexeme, "0.1");
    }

    #[test]
    fn test_keywords() {
        let source = "fn let if else while for";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let keywords: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            keywords,
            vec![
                TokenType::Fn,
                TokenType::Let,
                TokenType::If,
                TokenType::Else,
                TokenType::While,
                TokenType::For,
            ]
        );
    }

    #[test]
    fn test_operators() {
        let source = "+ - * / >= <= == != ->";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let operators: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            operators,
            vec![
                TokenType::Plus,
                TokenType::Minus,
                TokenType::Star,
                TokenType::Slash,
                TokenType::GreaterEqual,
                TokenType::LessEqual,
                TokenType::EqualEqual,
                TokenType::BangEqual,
                TokenType::Arrow,
            ]
        );
    }

    #[test]
    fn test_line_counting() {
        let source = "line1\nline2\n\nline4";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        // Find identifiers and check their line numbers
        let identifiers: Vec<(String, u32)> = tokens
            .iter()
            .filter(|t| t.kind == TokenType::Identifier)
            .map(|t| (t.lexeme.to_string(), t.line))
            .collect();

        assert_eq!(
            identifiers,
            vec![
                ("line1".to_string(), 1),
                ("line2".to_string(), 2),
                ("line4".to_string(), 4),
            ]
        );
    }

    #[test]
    fn test_comments() {
        let source = r#"// This is a comment
fn test() { // Another comment
    print("Hello"); // Comment after code
}"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let token_types: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            token_types,
            vec![
                TokenType::Fn,
                TokenType::Identifier,
                TokenType::OpenParen,
                TokenType::CloseParen,
                TokenType::OpenBrace,
                TokenType::Print,
                TokenType::OpenParen,
                TokenType::String,
                TokenType::CloseParen,
                TokenType::Semicolon,
                TokenType::CloseBrace,
            ]
        );
    }

    #[test]
    fn test_ai_keywords() {
        let source = "ai agent prompt";
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let keywords: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            keywords,
            vec![TokenType::AI, TokenType::Agent, TokenType::Prompt,]
        );
    }

    #[test]
    fn test_mixed_tokens() {
        let source = r#"fn calculate(x: number) {
    let result = x * 2;
    return result;
}"#;
        let scanner = Scanner::new(source);
        let tokens: Vec<Token> = scanner.collect();

        let token_types: Vec<TokenType> = tokens
            .iter()
            .map(|t| t.kind)
            .filter(|k| *k != TokenType::Eof)
            .collect();

        assert_eq!(
            token_types,
            vec![
                TokenType::Fn,
                TokenType::Identifier, // calculate
                TokenType::OpenParen,
                TokenType::Identifier, // x
                TokenType::Colon,
                TokenType::Identifier, // number
                TokenType::CloseParen,
                TokenType::OpenBrace,
                TokenType::Let,
                TokenType::Identifier, // result
                TokenType::Equal,
                TokenType::Identifier, // x
                TokenType::Star,
                TokenType::Number, // 2
                TokenType::Semicolon,
                TokenType::Return,
                TokenType::Identifier, // result
                TokenType::Semicolon,
                TokenType::CloseBrace,
            ]
        );
    }
}
