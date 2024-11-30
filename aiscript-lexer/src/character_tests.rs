#[cfg(test)]
mod tests {
    use crate::{Lexer, TokenType};

    #[test]
    fn test_mixed_utf8_string() {
        let source = r#""Hello 你好 World 世界""#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::String);
        assert_eq!(token.lexeme, r#""Hello 你好 World 世界""#);
    }

    #[test]
    fn test_chinese_identifier() {
        let source = "let 变量 = 42;";
        let mut scanner = Lexer::new(source);

        let expected_tokens = vec![
            TokenType::Let,
            TokenType::Identifier, // 变量
            TokenType::Equal,
            TokenType::Number, // 42
            TokenType::Semicolon,
            TokenType::Eof,
        ];

        for expected in expected_tokens {
            let token = scanner.next().unwrap();
            assert_eq!(token.kind, expected);
        }
    }

    #[test]
    fn test_utf8_in_comments() {
        let source = "// 这是一个注释\nprint(x);";
        let mut scanner = Lexer::new(source);

        let expected_tokens = vec![
            TokenType::Identifier,
            TokenType::OpenParen,
            TokenType::Identifier, // x
            TokenType::CloseParen,
            TokenType::Semicolon,
            TokenType::Eof,
        ];

        for expected in expected_tokens {
            let token = scanner.next().unwrap();
            assert_eq!(token.kind, expected);
        }
    }

    #[test]
    fn test_utf8_in_docstring() {
        let source = r#"
            """
            这是一个文档字符串，
            包含多行中文内容。
            """
        "#;
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::Doc);
    }

    #[test]
    fn test_complex_utf8_tokens() {
        let source = r#"
            let user = {
                name: "张三",
                age: 25,
                greet: fn() {
                    print("你好，" + self.name + "！");
                }
            };
        "#;
        let mut scanner = Lexer::new(source);

        let expected_tokens = vec![
            TokenType::Let,
            TokenType::Identifier, // user
            TokenType::Equal,
            TokenType::OpenBrace,
            TokenType::Identifier, // name
            TokenType::Colon,
            TokenType::String, // "张三"
            TokenType::Comma,
            TokenType::Identifier, // age
            TokenType::Colon,
            TokenType::Number, // 25
            TokenType::Comma,
            TokenType::Identifier, // greet
            TokenType::Colon,
            TokenType::Fn,
            TokenType::OpenParen,
            TokenType::CloseParen,
            TokenType::OpenBrace,
            TokenType::Identifier,
            TokenType::OpenParen,
            TokenType::String, // "你好，"
            TokenType::Plus,
            TokenType::Self_,
            TokenType::Dot,
            TokenType::Identifier, // name
            TokenType::Plus,
            TokenType::String, // "！"
            TokenType::CloseParen,
            TokenType::Semicolon,
            TokenType::CloseBrace,
            TokenType::CloseBrace,
            TokenType::Semicolon,
            TokenType::Eof,
        ];

        for expected in expected_tokens {
            let token = scanner.next().unwrap();
            assert_eq!(token.kind, expected);
        }
    }

    #[test]
    fn test_check_next_with_utf8() {
        let source = "你好世界";
        let scanner = Lexer::new(source);

        assert_eq!(scanner.check_next(2), Some("你好"));
        assert_eq!(scanner.check_next(3), Some("你好世"));
        assert_eq!(scanner.check_next(4), Some("你好世界"));
        assert_eq!(scanner.check_next(5), None);
    }

    #[test]
    fn test_scanner_line_counting_with_utf8() {
        let source = "print \"你好\"\n print \"世界\"\n";
        let mut scanner = Lexer::new(source);

        // First line
        let token = scanner.next().unwrap();
        assert_eq!(token.line, 1);
        let token = scanner.next().unwrap();
        assert_eq!(token.line, 1);

        // Second line
        let token = scanner.next().unwrap(); // print
        assert_eq!(token.line, 2);
        let token = scanner.next().unwrap(); // "世界"
        assert_eq!(token.line, 2);
    }

    #[test]
    fn test_error_handling_with_utf8() {
        let source = "\"未终止的字符串";
        let mut scanner = Lexer::new(source);

        let token = scanner.next().unwrap();
        assert_eq!(token.kind, TokenType::Error);
        assert_eq!(token.lexeme, "Unterminated string.");
    }
}
