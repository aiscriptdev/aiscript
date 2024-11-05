use std::{collections::HashMap, ops::Add};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    ast::{Expr, LiteralValue, Program, Stmt},
    lexer::{Scanner, Token, TokenType},
    object::FunctionType,
    vm::Context,
    VmError,
};

type ParseFn<'gc> = fn(&mut Parser<'gc>, bool /*can assign*/) -> Option<Expr<'gc>>;

pub struct Parser<'gc> {
    ctx: Context<'gc>,
    scanner: Scanner<'gc>,
    current: Token<'gc>,
    previous: Token<'gc>,
    previous_expr: Option<Expr<'gc>>,
    fn_type: FunctionType,
    class_compiler: Option<Box<ClassCompiler>>,
    had_error: bool,
    panic_mode: bool,
}

#[derive(Default)]
struct ClassCompiler {
    has_superclass: bool,
    enclosing: Option<Box<ClassCompiler>>,
}

impl<'gc> Parser<'gc> {
    pub fn new(ctx: Context<'gc>, source: &'gc str) -> Self {
        Parser {
            ctx,
            scanner: Scanner::new(source),
            current: Token::default(),
            previous: Token::default(),
            previous_expr: None,
            fn_type: FunctionType::Script,
            class_compiler: None,
            had_error: false,
            panic_mode: false,
        }
    }

    pub fn parse(&mut self) -> Result<Program<'gc>, VmError> {
        let mut program = Program::new();
        self.advance();

        while !self.is_at_end() {
            if let Some(stmt) = self.declaration() {
                program.statements.push(stmt);
            }
        }

        if self.had_error {
            Err(VmError::CompileError)
        } else {
            Ok(program)
        }
    }

    fn declaration(&mut self) -> Option<Stmt<'gc>> {
        let stmt = if self.match_token(TokenType::Class) {
            self.class_declaration()
        } else if self.match_token(TokenType::AI) {
            self.consume(TokenType::Fn, "Expect 'fn' after 'ai'.");
            self.func_declaration(FunctionType::AiFunction)
        } else if self.match_token(TokenType::Fn) {
            self.func_declaration(FunctionType::Function)
        } else if self.match_token(TokenType::Let) {
            self.var_declaration()
        } else if self.match_token(TokenType::Agent) {
            self.agent_declaration()
        } else {
            self.statement()
        };

        if self.panic_mode {
            self.synchronize();
            None
        } else {
            stmt
        }
    }

    fn statement(&mut self) -> Option<Stmt<'gc>> {
        if self.match_token(TokenType::Print) {
            self.print_statement()
        } else if self.match_token(TokenType::OpenBrace) {
            Some(Stmt::Block {
                statements: self.block(),
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

    fn agent_declaration(&mut self) -> Option<Stmt<'gc>> {
        self.consume(TokenType::Identifier, "Expect agent name.");
        let name = self.previous;

        self.consume(TokenType::OpenBrace, "Expect '{' before agent body.");
        let mut fields = HashMap::new();
        while !self.check(TokenType::CloseBrace) && !self.is_at_end() {
            let (key, value) = self.field_declaration()?;
            match key.lexeme {
                "instructions" | "model" | "tool_choice" => {
                    if !matches!(
                        value,
                        Expr::Literal {
                            value: LiteralValue::String { .. },
                            ..
                        }
                    ) {
                        self.error(&format!(
                            "Field '{}' in agent declaration should be a string.",
                            key.lexeme
                        ));
                        continue;
                    }
                }
                "tools" => {
                    if !matches!(value, Expr::Array { .. }) {
                        self.error("Field 'tools' in agent declaration should be an array.");
                        continue;
                    }
                }
                invalid => self.error_at(
                    key,
                    &format!("Invalid field '{}' in agent declaration.", invalid),
                ),
            }
            let field_name = self.ctx.intern(key.lexeme.as_bytes());

            if fields.contains_key(&field_name) {
                self.error_at(
                    key,
                    &format!("Duplicate field '{}' in agent declaration.", key.lexeme),
                );
                continue;
            }
            fields.insert(self.ctx.intern(key.lexeme.as_bytes()), value);
            // Consume comma after field declaration
            if !self.check(TokenType::CloseBrace) {
                self.consume(TokenType::Comma, "Expect ',' after field declaration.");
            }
        }

        // Check for required fields
        let required_fields = ["instructions"];
        for field in required_fields.iter() {
            if !fields.contains_key(&self.ctx.intern(field.as_bytes())) {
                self.error(&format!(
                    "Missing required field '{}' in agent declaration.",
                    field
                ));
                return None;
            }
        }

        self.consume(TokenType::CloseBrace, "Expect '}' after agent body.");
        Some(Stmt::Agent {
            name,
            fields,
            line: name.line,
        })
    }

    fn field_declaration(&mut self) -> Option<(Token<'gc>, Expr<'gc>)> {
        self.consume(TokenType::Identifier, "Expect field name.");
        let key = self.previous;
        self.consume(TokenType::Colon, "Expect ':' after field name.");
        let value = self.expression()?;
        Some((key, value))
    }

    fn class_declaration(&mut self) -> Option<Stmt<'gc>> {
        self.consume(TokenType::Identifier, "Expect class name.");
        let name = self.previous;
        let superclass = if self.match_token(TokenType::Less) {
            self.consume(TokenType::Identifier, "Expect superclass name.");
            let superclass_name = self.previous;
            if name.lexeme == superclass_name.lexeme {
                self.error("A class can't inherit from itself.");
            }
            Some(Expr::Variable {
                name: superclass_name,
                line: superclass_name.line,
            })
        } else {
            None
        };

        let class_compiler = ClassCompiler {
            has_superclass: superclass.is_some(),
            enclosing: self.class_compiler.take(),
        };
        self.class_compiler = Some(Box::new(class_compiler));

        self.consume(TokenType::OpenBrace, "Expect '{' before class body.");

        let mut methods = Vec::new();
        while !self.check(TokenType::CloseBrace) && !self.is_at_end() {
            methods.push(self.func_declaration(FunctionType::Method)?);
        }

        self.consume(TokenType::CloseBrace, "Expect '}' after class body.");

        // pop that compiler off the stack and restore the enclosing class compiler.
        self.class_compiler = self.class_compiler.take().and_then(|c| c.enclosing);
        Some(Stmt::Class {
            name,
            superclass,
            methods,
            line: name.line,
        })
    }

    fn func_declaration(&mut self, fn_type: FunctionType) -> Option<Stmt<'gc>> {
        // Save current function type
        let previous_fn_type = self.fn_type;
        self.fn_type = fn_type;
        let type_name = match fn_type {
            FunctionType::Method => "method",
            _ => "function",
        };

        self.consume(TokenType::Identifier, &format!("Expect {type_name} name."));
        let name = self.previous;
        if self.fn_type == FunctionType::Method && name.lexeme == "init" {
            self.fn_type = FunctionType::Initializer;
        }
        self.consume(TokenType::OpenParen, "Expect '(' after function name.");

        let mut params = Vec::new();
        if !self.check(TokenType::CloseParen) {
            loop {
                if params.len() >= 255 {
                    self.error_at_current("Can't have more than 255 parameters.");
                }

                self.consume(TokenType::Identifier, "Expect parameter name.");
                params.push(self.previous);

                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::CloseParen, "Expect ')' after parameters.");
        self.consume(TokenType::OpenBrace, "Expect '{' before function body.");
        let body = self.block();
        // Restore previous function type
        self.fn_type = previous_fn_type;
        Some(Stmt::Function {
            name,
            params,
            body,
            is_ai: fn_type == FunctionType::AiFunction,
            line: name.line,
        })
    }

    fn var_declaration(&mut self) -> Option<Stmt<'gc>> {
        self.consume(TokenType::Identifier, "Expect variable name.");
        let name = self.previous;

        let initializer = if self.match_token(TokenType::Equal) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            TokenType::Semicolon,
            "Expect ';' after variable declaration.",
        );
        Some(Stmt::Let {
            name,
            initializer,
            line: name.line,
        })
    }

    fn while_statement(&mut self) -> Option<Stmt<'gc>> {
        self.consume(TokenType::OpenParen, "Expect '(' after 'while'.");
        let condition = self.expression()?;
        self.consume(TokenType::CloseParen, "Expect ')' after condition.");
        let body = Box::new(self.statement()?);

        Some(Stmt::Loop {
            condition,
            body,
            line: self.previous.line,
        })
    }

    fn for_statement(&mut self) -> Option<Stmt<'gc>> {
        self.consume(TokenType::OpenParen, "Expect '(' after 'for'.");

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
        self.consume(TokenType::Semicolon, "Expect ';' after loop condition.");

        let increment = if !self.check(TokenType::CloseParen) {
            Some(self.expression()?)
        } else {
            None
        };
        self.consume(TokenType::CloseParen, "Expect ')' after for clauses.");

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

        body = Stmt::Loop {
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

        Some(body)
    }

    fn if_statement(&mut self) -> Option<Stmt<'gc>> {
        self.consume(TokenType::OpenParen, "Expect '(' after 'if'.");
        let condition = self.expression()?;
        self.consume(TokenType::CloseParen, "Expect ')' after condition.");

        let then_branch = Box::new(self.statement()?);
        let else_branch = if self.match_token(TokenType::Else) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };

        Some(Stmt::If {
            condition,
            then_branch,
            else_branch,
            line: self.previous.line,
        })
    }

    fn print_statement(&mut self) -> Option<Stmt<'gc>> {
        let value = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after value.");
        Some(Stmt::Print {
            expression: value,
            line: self.previous.line,
        })
    }

    fn return_statement(&mut self) -> Option<Stmt<'gc>> {
        let value = if !self.check(TokenType::Semicolon) {
            if self.fn_type == FunctionType::Initializer {
                self.error("Can't return a value from an initializer.");
            }
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(TokenType::Semicolon, "Expect ';' after return value.");
        Some(Stmt::Return {
            value,
            line: self.previous.line,
        })
    }

    fn expression_statement(&mut self) -> Option<Stmt<'gc>> {
        let expr = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after expression.");
        Some(Stmt::Expression {
            expression: expr,
            line: self.previous.line,
        })
    }

    fn block(&mut self) -> Vec<Stmt<'gc>> {
        let mut statements = Vec::new();

        while !self.check(TokenType::CloseBrace) && !self.is_at_end() {
            if let Some(declaration) = self.declaration() {
                statements.push(declaration);
            }
        }

        self.consume(TokenType::CloseBrace, "Expect '}' after block.");
        statements
    }

    fn expression(&mut self) -> Option<Expr<'gc>> {
        self.parse_precedence(Precedence::Assignment)
    }

    fn number(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let value = self.previous.lexeme.parse::<f64>().unwrap();
        Some(Expr::Literal {
            value: LiteralValue::Number(value),
            line: self.previous.line,
        })
    }

    fn string(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let string = self.previous.lexeme.trim_matches('"');
        Some(Expr::Literal {
            value: LiteralValue::String(self.ctx.intern(string.as_bytes())),
            line: self.previous.line,
        })
    }

    fn literal(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        match self.previous.kind {
            TokenType::False => Some(Expr::Literal {
                value: LiteralValue::Boolean(false),
                line: self.previous.line,
            }),
            TokenType::True => Some(Expr::Literal {
                value: LiteralValue::Boolean(true),
                line: self.previous.line,
            }),
            TokenType::Nil => Some(Expr::Literal {
                value: LiteralValue::Nil,
                line: self.previous.line,
            }),
            _ => unreachable!(),
        }
    }

    fn grouping(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let expr = self.expression()?;
        self.consume(TokenType::CloseParen, "Expect ')' after expression.");
        Some(Expr::Grouping {
            expression: Box::new(expr),
            line: self.previous.line,
        })
    }

    fn unary(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let operator = self.previous;
        let right = Box::new(self.parse_precedence(Precedence::Unary)?);
        Some(Expr::Unary {
            operator,
            right,
            line: operator.line,
        })
    }

    fn binary(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let operator = self.previous;
        let rule = get_rule(operator.kind);
        let left = Box::new(self.previous_expr.take()?);
        let right = Box::new(self.parse_precedence(rule.precedence + 1)?);

        Some(Expr::Binary {
            left,
            operator,
            right,
            line: operator.line,
        })
    }

    fn and(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let left = Box::new(self.previous_expr.take()?);
        let right = Box::new(self.parse_precedence(Precedence::And)?);
        Some(Expr::And {
            left,
            right,
            line: self.previous.line,
        })
    }

    fn or(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let left = Box::new(self.previous_expr.take()?);
        let right = Box::new(self.parse_precedence(Precedence::Or)?);
        Some(Expr::Or {
            left,
            right,
            line: self.previous.line,
        })
    }

    fn argument_list(&mut self) -> Option<Vec<Expr<'gc>>> {
        let mut arguments = Vec::new();

        if !self.check(TokenType::CloseParen) {
            loop {
                arguments.push(self.expression()?);
                if arguments.len() > 255 {
                    self.error("Can't have more than 255 arguments.");
                    break;
                }
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }

        Some(arguments)
    }

    fn array(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let mut elements = Vec::new();
        let line = self.previous.line;

        if !self.check(TokenType::CloseBracket) {
            loop {
                elements.push(self.expression()?);

                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }

        self.consume(TokenType::CloseBracket, "Expect ']' after array elements.");

        Some(Expr::Array { elements, line })
    }

    fn call(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let callee = Box::new(self.previous_expr.take()?);

        let arguments = self.argument_list()?;
        self.consume(TokenType::CloseParen, "Expect ')' after arguments.");

        Some(Expr::Call {
            callee,
            arguments,
            line: self.previous.line,
        })
    }

    fn dot(&mut self, can_assign: bool) -> Option<Expr<'gc>> {
        self.consume(TokenType::Identifier, "Expect property name after '.'.");
        let name = self.previous;
        let object = Box::new(self.previous_expr.take()?);

        if can_assign && self.match_token(TokenType::Equal) {
            let value = Box::new(self.expression()?);
            Some(Expr::Set {
                object,
                name,
                value,
                line: self.previous.line,
            })
        } else if self.match_token(TokenType::OpenParen) {
            let arguments = self.argument_list()?;
            self.consume(TokenType::CloseParen, "Expect ')' after arguments.");

            Some(Expr::Invoke {
                object,
                method: name,
                arguments,
                line: self.previous.line,
            })
        } else {
            Some(Expr::Get {
                object,
                name,
                line: self.previous.line,
            })
        }
    }

    fn variable(&mut self, can_assign: bool) -> Option<Expr<'gc>> {
        let name = self.previous;

        if can_assign && self.match_token(TokenType::Equal) {
            let value = Box::new(self.expression()?);
            Some(Expr::Assign {
                name,
                value,
                line: self.previous.line,
            })
        } else {
            Some(Expr::Variable {
                name,
                line: name.line,
            })
        }
    }

    fn super_(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        if self.class_compiler.is_none() {
            self.error("Can't use 'super' outside of a class.");
        } else if self.class_compiler.as_ref().map(|c| c.has_superclass) == Some(false) {
            self.error("Can't use 'super' in a class with no superclass.");
        }

        let keyword = self.previous;
        self.consume(TokenType::Dot, "Expect '.' after 'super'.");
        self.consume(TokenType::Identifier, "Expect superclass method name.");
        let method = self.previous;

        if self.match_token(TokenType::OpenParen) {
            let arguments = self.argument_list()?;
            self.consume(TokenType::CloseParen, "Expect ')' after arguments.");

            Some(Expr::SuperInvoke {
                method,
                arguments,
                line: keyword.line,
            })
        } else {
            Some(Expr::Super {
                method,
                arguments: Vec::new(),
                line: keyword.line,
            })
        }
    }

    fn this(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        if self.class_compiler.is_none() {
            self.error("Can't use 'this' outside of a class.");
            return None;
        }

        Some(Expr::This {
            line: self.previous.line,
        })
    }

    fn prompt(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        if self.fn_type != FunctionType::AiFunction && self.fn_type != FunctionType::Script {
            self.error("Can't prompt outside of ai function or root.");
        }
        let expr = Box::new(self.expression()?);
        Some(Expr::Prompt {
            expression: expr,
            line: self.previous.line,
        })
    }

    // Pratt parsing implementation
    fn parse_precedence(&mut self, precedence: Precedence) -> Option<Expr<'gc>> {
        self.advance();
        let prefix_rule = get_rule(self.previous.kind).prefix;
        let can_assign = precedence <= Precedence::Assignment;

        let expr = if let Some(prefix_fn) = prefix_rule {
            (prefix_fn)(self, can_assign)
        } else {
            self.error("Expect expression.");
            return None;
        };

        self.previous_expr = expr;

        while precedence <= get_rule(self.current.kind).precedence {
            self.advance();
            let infix_rule = get_rule(self.previous.kind).infix;
            if let Some(infix_fn) = infix_rule {
                let expr = (infix_fn)(self, can_assign)?;
                self.previous_expr = Some(expr);
            }
        }

        let expr = self.previous_expr.take()?;
        if can_assign && self.match_token(TokenType::Equal) {
            self.error("Invalid assignment target.");
        }
        Some(expr)
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

    fn consume(&mut self, kind: TokenType, message: &str) {
        if self.check(kind) {
            self.advance();
            return;
        }
        self.error_at_current(message);
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

    fn error_at_current(&mut self, message: &str) {
        self.error_at(self.current, message);
    }

    fn error(&mut self, message: &str) {
        self.error_at(self.previous, message);
    }

    fn error_at(&mut self, token: Token<'gc>, message: &str) {
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
        TokenType::OpenParen => {
            ParseRule::new(Some(Parser::grouping), Some(Parser::call), Precedence::Call)
        }
        TokenType::OpenBracket => ParseRule::new(Some(Parser::array), None, Precedence::Call),
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

#[cfg(test)]
mod tests {
    use gc_arena::arena::rootless_mutate;

    use super::*;
    use crate::{string::InternedStringSet, vm::Context};

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
            let Stmt::Agent { name, fields, line } = &result.statements[0] else {
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
            let Stmt::Agent { name, fields, .. } = &result.statements[0] else {
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
