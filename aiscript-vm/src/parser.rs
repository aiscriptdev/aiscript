use std::ops::Add;

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    ast::{Expr, LiteralValue, ParseError, Program, Stmt},
    object::FunctionType,
    scanner::{Scanner, Token, TokenType},
    vm::Context,
};

type ParseFn<'gc> = fn(&mut Parser<'gc>, bool /*can assign*/) -> Result<Expr<'gc>, ParseError>;

pub struct Parser<'gc> {
    ctx: Context<'gc>,
    scanner: Scanner<'gc>,
    current: Token<'gc>,
    previous: Token<'gc>,
    previous_expr: Option<Expr<'gc>>,
    had_error: bool,
    panic_mode: bool,
}

impl<'gc> Parser<'gc> {
    pub fn new(ctx: Context<'gc>, source: &'gc str) -> Self {
        Parser {
            ctx,
            scanner: Scanner::new(source),
            current: Token::default(),
            previous: Token::default(),
            previous_expr: None,
            had_error: false,
            panic_mode: false,
        }
    }

    pub fn parse(&mut self) -> Result<Program<'gc>, ParseError> {
        let mut program = Program::new();
        self.advance();

        while !self.is_at_end() {
            if let Some(stmt) = self.declaration()? {
                program.statements.push(stmt);
            }
        }

        if self.had_error {
            Err(ParseError::new("Failed to parse program", 0))
        } else {
            Ok(program)
        }
    }

    fn declaration(&mut self) -> Result<Option<Stmt<'gc>>, ParseError> {
        let stmt = if self.match_token(TokenType::Class) {
            self.class_declaration()
        } else if self.match_token(TokenType::AI) {
            self.consume(TokenType::Fn, "Expect 'fn' after 'ai'.")?;
            self.function(FunctionType::AiFunction)
        } else if self.match_token(TokenType::Fn) {
            self.function(FunctionType::Function)
        } else if self.match_token(TokenType::Let) {
            self.var_declaration()
        } else {
            self.statement()
        };

        if self.panic_mode {
            self.synchronize();
            Ok(None)
        } else {
            stmt.map(Some)
        }
    }

    fn statement(&mut self) -> Result<Stmt<'gc>, ParseError> {
        if self.match_token(TokenType::Print) {
            self.print_statement()
        } else if self.match_token(TokenType::LeftBrace) {
            Ok(Stmt::Block {
                statements: self.block()?,
                line: self.previous.line,
            })
        } else if self.match_token(TokenType::If) {
            self.if_statement()
        } else if self.match_token(TokenType::Return) {
            self.return_statement()
        } else if self.match_token(TokenType::While) {
            self.while_statement()
        } else if self.match_token(TokenType::For) {
            self.for_statement()
        } else {
            self.expression_statement()
        }
    }

    fn class_declaration(&mut self) -> Result<Stmt<'gc>, ParseError> {
        let name = self.consume(TokenType::Identifier, "Expect class name.")?;
        let superclass = if self.match_token(TokenType::Less) {
            let superclass_name = self.consume(TokenType::Identifier, "Expect superclass name.")?;
            if name.lexeme == superclass_name.lexeme {
                return Err(ParseError::new(
                    "A class can't inherit from itself.",
                    name.line,
                ));
            }
            Some(Expr::Variable {
                name: superclass_name,
                line: superclass_name.line,
            })
        } else {
            None
        };

        self.consume(TokenType::LeftBrace, "Expect '{' before class body.")?;

        let mut methods = Vec::new();
        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            methods.push(self.function(FunctionType::Method)?);
        }

        self.consume(TokenType::RightBrace, "Expect '}' after class body.")?;

        Ok(Stmt::Class {
            name,
            superclass,
            methods,
            line: name.line,
        })
    }

    fn function(&mut self, fn_type: FunctionType) -> Result<Stmt<'gc>, ParseError> {
        let type_name = match fn_type {
            FunctionType::Method => "method",
            _ => "function",
        };
        let name = self.consume(TokenType::Identifier, &format!("Expect {type_name} name."))?;
        self.consume(TokenType::LeftParen, "Expect '(' after function name.")?;

        let mut params = Vec::new();
        if !self.check(TokenType::RightParen) {
            loop {
                if params.len() >= 255 {
                    return Err(ParseError::new(
                        "Can't have more than 255 parameters.",
                        self.peek().line,
                    ));
                }

                params.push(self.consume(TokenType::Identifier, "Expect parameter name.")?);

                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.")?;
        self.consume(TokenType::LeftBrace, "Expect '{' before function body.")?;
        let body = self.block()?;
        Ok(Stmt::Function {
            name,
            params,
            body,
            is_ai: fn_type == FunctionType::AiFunction,
            line: name.line,
        })
    }

    fn var_declaration(&mut self) -> Result<Stmt<'gc>, ParseError> {
        let name = self.consume(TokenType::Identifier, "Expect variable name.")?;

        let initializer = if self.match_token(TokenType::Equal) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            TokenType::Semicolon,
            "Expect ';' after variable declaration.",
        )?;
        Ok(Stmt::Let {
            name,
            initializer,
            line: name.line,
        })
    }

    fn while_statement(&mut self) -> Result<Stmt<'gc>, ParseError> {
        self.consume(TokenType::LeftParen, "Expect '(' after 'while'.")?;
        let condition = self.expression()?;
        self.consume(TokenType::RightParen, "Expect ')' after condition.")?;
        let body = Box::new(self.statement()?);

        Ok(Stmt::While {
            condition,
            body,
            line: self.previous.line,
        })
    }

    fn for_statement(&mut self) -> Result<Stmt<'gc>, ParseError> {
        self.consume(TokenType::LeftParen, "Expect '(' after 'for'.")?;

        let initializer = if self.match_token(TokenType::Semicolon) {
            None
        } else if self.match_token(TokenType::Let) {
            Some(self.var_declaration()?)
        } else {
            Some(self.expression_statement()?)
        };

        let condition = if !self.check(TokenType::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };
        self.consume(TokenType::Semicolon, "Expect ';' after loop condition.")?;

        let increment = if !self.check(TokenType::RightParen) {
            Some(self.expression()?)
        } else {
            None
        };
        self.consume(TokenType::RightParen, "Expect ')' after for clauses.")?;

        let mut body = self.statement()?;

        // Desugar for loop into while loop
        if let Some(increment) = increment {
            body = Stmt::Block {
                statements: vec![
                    body,
                    Stmt::Expression {
                        expression: increment,
                        line: self.previous.line,
                    },
                ],
                line: self.previous.line,
            };
        }

        body = Stmt::While {
            condition: condition.unwrap_or(Expr::Literal {
                value: LiteralValue::Boolean(true),
                line: self.previous.line,
            }),
            body: Box::new(body),
            line: self.previous.line,
        };

        if let Some(initializer) = initializer {
            body = Stmt::Block {
                statements: vec![initializer, body],
                line: self.previous.line,
            };
        }

        Ok(body)
    }

    fn if_statement(&mut self) -> Result<Stmt<'gc>, ParseError> {
        self.consume(TokenType::LeftParen, "Expect '(' after 'if'.")?;
        let condition = self.expression()?;
        self.consume(TokenType::RightParen, "Expect ')' after condition.")?;

        let then_branch = Box::new(self.statement()?);
        let else_branch = if self.match_token(TokenType::Else) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };

        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
            line: self.previous.line,
        })
    }

    fn print_statement(&mut self) -> Result<Stmt<'gc>, ParseError> {
        let value = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after value.")?;
        Ok(Stmt::Print {
            expression: value,
            line: self.previous.line,
        })
    }

    fn return_statement(&mut self) -> Result<Stmt<'gc>, ParseError> {
        let value = if !self.check(TokenType::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(TokenType::Semicolon, "Expect ';' after return value.")?;
        Ok(Stmt::Return {
            value,
            line: self.previous.line,
        })
    }

    fn expression_statement(&mut self) -> Result<Stmt<'gc>, ParseError> {
        let expr = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after expression.")?;
        Ok(Stmt::Expression {
            expression: expr,
            line: self.previous.line,
        })
    }

    fn block(&mut self) -> Result<Vec<Stmt<'gc>>, ParseError> {
        let mut statements = Vec::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            if let Some(declaration) = self.declaration()? {
                statements.push(declaration);
            }
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.")?;
        Ok(statements)
    }

    fn expression(&mut self) -> Result<Expr<'gc>, ParseError> {
        self.parse_precedence(Precedence::Assignment)
    }

    fn number(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let value = self.previous.lexeme.parse::<f64>().unwrap();
        Ok(Expr::Literal {
            value: LiteralValue::Number(value),
            line: self.previous.line,
        })
    }

    fn string(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let string = self.previous.lexeme.trim_matches('"');
        Ok(Expr::Literal {
            value: LiteralValue::String(self.ctx.intern(string.as_bytes())),
            line: self.previous.line,
        })
    }

    fn literal(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        match self.previous.kind {
            TokenType::False => Ok(Expr::Literal {
                value: LiteralValue::Boolean(false),
                line: self.previous.line,
            }),
            TokenType::True => Ok(Expr::Literal {
                value: LiteralValue::Boolean(true),
                line: self.previous.line,
            }),
            TokenType::Nil => Ok(Expr::Literal {
                value: LiteralValue::Nil,
                line: self.previous.line,
            }),
            _ => unreachable!(),
        }
    }

    fn grouping(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let expr = self.expression()?;
        self.consume(TokenType::RightParen, "Expect ')' after expression.")?;
        Ok(Expr::Grouping {
            expression: Box::new(expr),
            line: self.previous.line,
        })
    }

    fn unary(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let operator = self.previous;
        let right = Box::new(self.parse_precedence(Precedence::Unary)?);
        Ok(Expr::Unary {
            operator,
            right,
            line: operator.line,
        })
    }

    fn binary(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let operator = self.previous;
        let rule = get_rule(operator.kind);
        let left = Box::new(self.previous_expr.take().unwrap());
        let right = Box::new(self.parse_precedence(rule.precedence + 1)?);

        Ok(Expr::Binary {
            left,
            operator,
            right,
            line: operator.line,
        })
    }

    fn and(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let left = Box::new(self.previous_expr.take().unwrap());
        let right = Box::new(self.parse_precedence(Precedence::And)?);
        Ok(Expr::And {
            left,
            right,
            line: self.previous.line,
        })
    }

    fn or(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let left = Box::new(self.previous_expr.take().unwrap());
        let right = Box::new(self.parse_precedence(Precedence::Or)?);
        Ok(Expr::Or {
            left,
            right,
            line: self.previous.line,
        })
    }

    fn call(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let mut arguments = Vec::new();
        let callee = Box::new(self.previous_expr.take().unwrap());

        if !self.check(TokenType::RightParen) {
            loop {
                if arguments.len() >= 255 {
                    return Err(ParseError::new(
                        "Can't have more than 255 arguments.",
                        self.peek().line,
                    ));
                }
                arguments.push(self.expression()?);
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after arguments.")?;

        Ok(Expr::Call {
            callee,
            arguments,
            line: self.previous.line,
        })
    }

    fn dot(&mut self, can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        self.consume(TokenType::Identifier, "Expect property name after '.'.")?;
        let name = self.previous;
        let object = Box::new(self.previous_expr.take().unwrap());

        if can_assign && self.match_token(TokenType::Equal) {
            let value = Box::new(self.expression()?);
            Ok(Expr::Set {
                object,
                name,
                value,
                line: self.previous.line,
            })
        } else if self.match_token(TokenType::LeftParen) {
            let mut arguments = Vec::new();
            if !self.check(TokenType::RightParen) {
                loop {
                    arguments.push(self.expression()?);
                    if !self.match_token(TokenType::Comma) {
                        break;
                    }
                }
            }
            self.consume(TokenType::RightParen, "Expect ')' after arguments.")?;

            Ok(Expr::Invoke {
                object,
                method: name,
                arguments,
                line: self.previous.line,
            })
        } else {
            Ok(Expr::Get {
                object,
                name,
                line: self.previous.line,
            })
        }
    }

    fn variable(&mut self, can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let name = self.previous;

        if can_assign && self.match_token(TokenType::Equal) {
            let value = Box::new(self.expression()?);
            Ok(Expr::Assign {
                name,
                value,
                line: self.previous.line,
            })
        } else {
            Ok(Expr::Variable {
                name,
                line: name.line,
            })
        }
    }

    fn super_(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let keyword = self.previous;
        self.consume(TokenType::Dot, "Expect '.' after 'super'.")?;
        self.consume(TokenType::Identifier, "Expect superclass method name.")?;
        let method = self.previous;

        Ok(Expr::Super {
            method,
            line: keyword.line,
        })
    }

    fn this(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        Ok(Expr::This {
            line: self.previous.line,
        })
    }

    fn prompt(&mut self, _can_assign: bool) -> Result<Expr<'gc>, ParseError> {
        let expr = Box::new(self.expression()?);
        Ok(Expr::Prompt {
            expression: expr,
            line: self.previous.line,
        })
    }

    // Pratt parsing implementation
    fn parse_precedence(&mut self, precedence: Precedence) -> Result<Expr<'gc>, ParseError> {
        self.advance();
        let prefix_rule = get_rule(self.previous.kind).prefix;
        let can_assign = precedence <= Precedence::Assignment;

        let expr = if let Some(prefix_fn) = prefix_rule {
            (prefix_fn)(self, can_assign)
        } else {
            Err(ParseError::new("Expect expression.", self.previous.line))
        }?;

        self.previous_expr = Some(expr);

        while precedence <= get_rule(self.current.kind).precedence {
            self.advance();
            let infix_rule = get_rule(self.previous.kind).infix;
            if let Some(infix_fn) = infix_rule {
                let expr = (infix_fn)(self, can_assign)?;
                self.previous_expr = Some(expr);
            }
        }

        let expr = self.previous_expr.take().unwrap();
        if can_assign && self.match_token(TokenType::Equal) {
            return Err(ParseError::new(
                "Invalid assignment target.",
                self.previous.line,
            ));
        }
        Ok(expr)
    }

    // Helper methods
    fn advance(&mut self) {
        self.previous = std::mem::take(&mut self.current);

        while let Some(token) = self.scanner.next() {
            self.current = token;
            if self.current.kind != TokenType::Error {
                break;
            }
            self.error_at_current(self.current.lexeme);
        }
    }

    fn consume(&mut self, kind: TokenType, message: &str) -> Result<Token<'gc>, ParseError> {
        if self.check(kind) {
            self.advance();
            Ok(self.previous)
        } else {
            Err(ParseError::new(message, self.current.line))
        }
    }

    fn match_token(&mut self, kind: TokenType) -> bool {
        if !self.check(kind) {
            false
        } else {
            self.advance();
            true
        }
    }

    fn check(&self, kind: TokenType) -> bool {
        self.current.kind == kind
    }

    fn is_at_end(&self) -> bool {
        self.current.kind == TokenType::Eof
    }

    fn peek(&self) -> &Token<'gc> {
        &self.current
    }

    fn error_at_current(&mut self, message: &str) {
        self.error_at(self.current, message);
    }

    // fn error(&mut self, message: &str) {
    //     self.error_at(self.previous, message);
    // }

    fn error_at(&mut self, token: Token<'gc>, message: &str) {
        // if self.panic_mode {
        //     return Ok(());
        // }
        // self.panic_mode = true;
        // Err(ParseError::new(message, token.line))
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        eprint!("[line {}] Error", token.line);
        if token.kind == TokenType::Eof {
            eprint!(" at end");
        } else if token.kind == TokenType::Error {
            // Do nothing.
        } else {
            eprint!(" at '{}'", token.lexeme);
        }
        eprintln!(": {message}");
        self.had_error = true;
    }

    fn synchronize(&mut self) {
        self.panic_mode = false;

        while !self.is_at_end() {
            if self.previous.kind == TokenType::Semicolon {
                return;
            }

            match self.current.kind {
                TokenType::Class
                | TokenType::Fn
                | TokenType::Let
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => return,
                _ => self.advance(),
            }
        }
    }
}

// Precedence levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
enum Precedence {
    None,
    Assignment, // =
    Or,         // or
    And,        // and
    Equality,   // == !=
    Comparison, // < > <= >=
    Term,       // + -
    Factor,     // * /
    Unary,      // ! -
    Call,       // . ()
    Primary,
}

impl Add<u8> for Precedence {
    type Output = Self;

    fn add(self, rhs: u8) -> Self::Output {
        Self::try_from(self as u8 + rhs).unwrap()
    }
}

// Parse rule structure
struct ParseRule<'gc> {
    prefix: Option<ParseFn<'gc>>,
    infix: Option<ParseFn<'gc>>,
    precedence: Precedence,
}

impl<'gc> ParseRule<'gc> {
    fn new(
        prefix: Option<ParseFn<'gc>>,
        infix: Option<ParseFn<'gc>>,
        precedence: Precedence,
    ) -> Self {
        Self {
            prefix,
            infix,
            precedence,
        }
    }
}

fn get_rule<'gc>(kind: TokenType) -> ParseRule<'gc> {
    match kind {
        TokenType::LeftParen => {
            ParseRule::new(Some(Parser::grouping), Some(Parser::call), Precedence::Call)
        }
        TokenType::Dot => ParseRule::new(None, Some(Parser::dot), Precedence::Call),
        TokenType::Minus => {
            ParseRule::new(Some(Parser::unary), Some(Parser::binary), Precedence::Term)
        }
        TokenType::Plus => ParseRule::new(None, Some(Parser::binary), Precedence::Term),
        TokenType::Slash => ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
        TokenType::Star => ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
        TokenType::Bang => ParseRule::new(Some(Parser::unary), None, Precedence::None),
        TokenType::BangEqual => ParseRule::new(None, Some(Parser::binary), Precedence::Equality),
        TokenType::EqualEqual => ParseRule::new(None, Some(Parser::binary), Precedence::Equality),
        TokenType::Greater => ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        TokenType::GreaterEqual => {
            ParseRule::new(None, Some(Parser::binary), Precedence::Comparison)
        }
        TokenType::Less => ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        TokenType::LessEqual => ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        TokenType::Identifier => ParseRule::new(Some(Parser::variable), None, Precedence::None),
        TokenType::String => ParseRule::new(Some(Parser::string), None, Precedence::None),
        TokenType::Number => ParseRule::new(Some(Parser::number), None, Precedence::None),
        TokenType::And => ParseRule::new(None, Some(Parser::and), Precedence::And),
        TokenType::Or => ParseRule::new(None, Some(Parser::or), Precedence::Or),
        TokenType::Super => ParseRule::new(Some(Parser::super_), None, Precedence::None),
        TokenType::This => ParseRule::new(Some(Parser::this), None, Precedence::None),
        TokenType::True | TokenType::False | TokenType::Nil => {
            ParseRule::new(Some(Parser::literal), None, Precedence::None)
        }
        TokenType::Prompt => ParseRule::new(Some(Parser::prompt), None, Precedence::None),
        _ => ParseRule::new(None, None, Precedence::None),
    }
}
