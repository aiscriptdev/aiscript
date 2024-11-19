use std::{collections::HashMap, iter::Peekable, mem, ops::Add};

use indexmap::IndexMap;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{
    ast::{Expr, Literal, Parameter, Program, Stmt},
    lexer::{Scanner, Token, TokenType},
};
use crate::{
    ast::{AgentDecl, ClassDecl, FunctionDecl, VariableDecl, Visibility},
    object::FunctionType,
    vm::Context,
    VmError,
};

mod expr_test;
mod stmt_test;

type ParseFn<'gc> = fn(&mut Parser<'gc>, bool /*can assign*/) -> Option<Expr<'gc>>;

pub struct Parser<'gc> {
    ctx: Context<'gc>,
    scanner: Peekable<Scanner<'gc>>,
    current: Token<'gc>,
    previous: Token<'gc>,
    previous_expr: Option<Expr<'gc>>,
    fn_type: FunctionType,
    class_compiler: Option<Box<ClassCompiler>>,
    scopes: Vec<String>,
    // track if we're inside a loop
    loop_depth: usize,
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
            scanner: Scanner::new(source).peekable(),
            current: Token::default(),
            previous: Token::default(),
            previous_expr: None,
            fn_type: FunctionType::Script,
            class_compiler: None,
            scopes: Vec::new(),
            loop_depth: 0,
            had_error: false,
            panic_mode: false,
        }
    }

    pub fn parse(&mut self) -> Result<Program<'gc>, VmError> {
        self.scopes.push(String::from("script"));
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

    fn peek_next(&mut self) -> Option<Token<'gc>> {
        self.scanner.peek().copied()
    }

    fn parse_type(&mut self) -> Token<'gc> {
        if !self.check(TokenType::Identifier) {
            self.error_at_current("Invalid type annotation.");
        }
        // Parse either builtin type or custom type (identifier)
        self.advance();
        self.previous
    }

    fn parse_literal(&mut self) -> Option<Option<Expr<'gc>>> {
        let expr = match self.current.kind {
            TokenType::Number => {
                self.match_token(TokenType::Number);
                Some(Expr::Literal {
                    value: Literal::Number(self.previous.lexeme.parse().unwrap()),
                    line: self.previous.line,
                })
            }
            TokenType::String => {
                self.match_token(TokenType::String);
                Some(Expr::Literal {
                    value: Literal::String(
                        self.ctx
                            .intern(self.previous.lexeme.trim_matches('"').as_bytes()),
                    ),
                    line: self.previous.line,
                })
            }
            TokenType::True => {
                self.match_token(TokenType::True);
                Some(Expr::Literal {
                    value: Literal::Boolean(true),
                    line: self.previous.line,
                })
            }
            TokenType::False => {
                self.match_token(TokenType::False);
                Some(Expr::Literal {
                    value: Literal::Boolean(false),
                    line: self.previous.line,
                })
            }
            TokenType::Nil => {
                self.match_token(TokenType::Nil);
                Some(Expr::Literal {
                    value: Literal::Nil,
                    line: self.previous.line,
                })
            }
            _ => None,
        };
        Some(expr)
    }

    fn declaration(&mut self) -> Option<Stmt<'gc>> {
        let visibility = if self.match_token(TokenType::Pub) {
            Visibility::Public
        } else {
            Visibility::Private
        };

        let stmt = if self.match_token(TokenType::Use) {
            if visibility == Visibility::Public {
                self.error("'pub' modifier cannot be used with 'use' statement.");
                None
            } else {
                self.use_declaration()
            }
        } else if self.match_token(TokenType::Class) {
            self.class_declaration(visibility)
        } else if self.match_token(TokenType::AI) {
            self.consume(TokenType::Fn, "Expect 'fn' after 'ai'.");
            self.func_declaration(FunctionType::AiFunction, visibility)
        } else if self.match_token(TokenType::Fn) {
            self.func_declaration(FunctionType::Function, visibility)
        } else if self.match_token(TokenType::Let) {
            self.var_declaration(visibility)
        } else if self.match_token(TokenType::Const) {
            self.const_declaration(visibility)
        } else if self.match_token(TokenType::Agent) {
            self.agent_declaration(visibility)
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

    fn use_declaration(&mut self) -> Option<Stmt<'gc>> {
        // Create a vector to store all parts of the module path
        let mut path_parts = Vec::new();

        self.consume(TokenType::Identifier, "Expect module name after 'use'.");
        path_parts.push(self.previous);

        // Handle dotted module paths (e.g., "std.math")
        while self.match_token(TokenType::Dot) {
            self.consume(TokenType::Identifier, "Expect identifier after '.'.");
            path_parts.push(self.previous);
        }

        self.consume(TokenType::Semicolon, "Expect ';' after module path.");

        // Combine all parts into a single module path
        let mut full_path = String::new();
        for (i, part) in path_parts.iter().enumerate() {
            if i > 0 {
                full_path.push('.');
            }
            full_path.push_str(part.lexeme);
        }

        // Create a new token with the full path
        let path = Token::new(TokenType::Identifier, full_path.leak(), path_parts[0].line);

        Some(Stmt::Use {
            path,
            line: path.line,
        })
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
        } else if self.match_token(TokenType::Break) {
            self.break_statement()
        } else if self.match_token(TokenType::Continue) {
            self.continue_statement()
        } else {
            self.expression_statement()
        }
    }

    fn break_statement(&mut self) -> Option<Stmt<'gc>> {
        if self.loop_depth == 0 {
            self.error("Can't use 'break' outside of a loop.");
            return None;
        }

        self.consume(TokenType::Semicolon, "Expect ';' after 'break'.");
        Some(Stmt::Break {
            line: self.previous.line,
        })
    }

    fn continue_statement(&mut self) -> Option<Stmt<'gc>> {
        if self.loop_depth == 0 {
            self.error("Can't use 'continue' outside of a loop.");
            return None;
        }

        self.consume(TokenType::Semicolon, "Expect ';' after 'continue'.");
        Some(Stmt::Continue {
            line: self.previous.line,
        })
    }

    fn agent_declaration(&mut self, visibility: Visibility) -> Option<Stmt<'gc>> {
        self.consume(TokenType::Identifier, "Expect agent name.");
        let name = self.previous;
        self.scopes.push(name.lexeme.to_owned());
        self.consume(TokenType::OpenBrace, "Expect '{' before agent body.");
        let mut fields = HashMap::new();
        while !self.check(TokenType::CloseBrace) && !self.check(TokenType::Fn) && !self.is_at_end()
        {
            let (key, value) = self.field_declaration()?;
            match key.lexeme {
                "instructions" | "model" | "tool_choice" => {
                    if !matches!(
                        value,
                        Expr::Literal {
                            value: Literal::String { .. },
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

            if fields.contains_key(key.lexeme) {
                self.error_at(
                    key,
                    &format!("Duplicate field '{}' in agent declaration.", key.lexeme),
                );
                continue;
            }
            fields.insert(key.lexeme, value);
            // Consume comma after field declaration
            if !self.check(TokenType::CloseBrace) {
                self.consume(TokenType::Comma, "Expect ',' after field declaration.");
            }
        }

        // Check for required fields
        let required_fields = ["instructions"];
        for field in required_fields {
            if !fields.contains_key(field) {
                self.error(&format!(
                    "Missing required field '{}' in agent declaration.",
                    field
                ));
                return None;
            }
        }

        let mut tools = Vec::new();
        while !self.check(TokenType::CloseBrace) && !self.is_at_end() {
            self.consume(TokenType::Fn, "Expect 'fn' keyword.");
            tools.push(self.func_declaration(FunctionType::Tool, Visibility::Private)?);
        }

        self.consume(TokenType::CloseBrace, "Expect '}' after agent body.");
        self.scopes.pop();
        Some(Stmt::Agent(AgentDecl {
            name,
            mangled_name: format!("{}${}", self.scopes.join("$"), name.lexeme),
            fields,
            tools,
            visibility,
            line: name.line,
        }))
    }

    fn field_declaration(&mut self) -> Option<(Token<'gc>, Expr<'gc>)> {
        self.consume(TokenType::Identifier, "Expect field name.");
        let key = self.previous;
        self.consume(TokenType::Colon, "Expect ':' after field name.");
        let value = self.expression()?;
        Some((key, value))
    }

    fn class_declaration(&mut self, visibility: Visibility) -> Option<Stmt<'gc>> {
        self.consume(TokenType::Identifier, "Expect class name.");
        let name = self.previous;
        self.scopes.push(name.lexeme.to_owned());
        let superclass = if self.match_token(TokenType::OpenParen) {
            self.consume(TokenType::Identifier, "Expect superclass name.");
            let superclass_name = self.previous;
            if name.lexeme == superclass_name.lexeme {
                self.error("A class can't inherit from itself.");
            }
            self.consume(TokenType::CloseParen, "Expect ')' after superclass name");
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
            let method_vis = if self.match_token(TokenType::Pub) {
                Visibility::Public
            } else {
                Visibility::Private
            };
            let method = if self.match_token(TokenType::AI) {
                self.consume(TokenType::Fn, "Expect 'fn' after 'ai'.");
                self.func_declaration(FunctionType::AiMethod, method_vis)?
            } else if self.match_token(TokenType::Fn) {
                self.func_declaration(FunctionType::Method, method_vis)?
            } else {
                self.error_at_current("Expect 'fn' or 'ai fn' modifier for method.");
                return None;
            };
            methods.push(method);
        }

        self.consume(TokenType::CloseBrace, "Expect '}' after class body.");

        self.scopes.pop();
        // pop that compiler off the stack and restore the enclosing class compiler.
        self.class_compiler = self.class_compiler.take().and_then(|c| c.enclosing);
        Some(Stmt::Class(ClassDecl {
            name,
            superclass,
            methods,
            visibility,
            line: name.line,
        }))
    }

    fn func_declaration(
        &mut self,
        fn_type: FunctionType,
        visibility: Visibility,
    ) -> Option<Stmt<'gc>> {
        // Save current function type
        let previous_fn_type = self.fn_type;
        self.fn_type = fn_type;
        let type_name = match fn_type {
            FunctionType::Method => "method",
            FunctionType::Tool => "tool function",
            _ => "function",
        };

        self.consume(TokenType::Identifier, &format!("Expect {type_name} name."));
        let name = self.previous;
        self.scopes.push(name.lexeme.to_string());
        if self.fn_type == FunctionType::Method && name.lexeme == "init" {
            self.fn_type = FunctionType::Constructor;
        }

        self.consume(TokenType::OpenParen, "Expect '(' after function name.");

        // Use IndexMap for parameters and their types
        // IndexMap is ordered by insertion order,
        // which is matter for function call
        let mut params = IndexMap::new();
        let mut keyword_args_count = 0;
        loop {
            if self.check(TokenType::CloseParen) {
                break;
            }
            if params.len() >= 255 {
                self.error_at_current("Can't have more than 255 parameters.");
            }

            self.consume(TokenType::Identifier, "Expect parameter name.");
            let param_name = self.previous;

            // Parse parameter type annotation
            let type_hint = if self.match_token(TokenType::Colon) {
                Some(self.parse_type())
            } else {
                None
            };

            // Parse default value if present - must be a literal
            let default_value = if self.match_token(TokenType::Equal) {
                match self.parse_literal()? {
                    Some(expr) => {
                        keyword_args_count += 1;
                        Some(expr)
                    }
                    None => {
                        self.error("Default value must be a literal.");
                        None
                    }
                }
            } else {
                if keyword_args_count > 0 {
                    self.error("Positional parameter must come before parameter with a default.");
                }
                None
            };

            params.insert(
                param_name,
                Parameter {
                    name: param_name,
                    type_hint,
                    default_value,
                },
            );

            if !self.match_token(TokenType::Comma) {
                break;
            }
        }
        self.consume(TokenType::CloseParen, "Expect ')' after parameters.");

        // Parse optional return type
        let return_type = if self.match_token(TokenType::Arrow) {
            Some(self.parse_type())
        } else {
            None
        };
        self.consume(TokenType::OpenBrace, "Expect '{' before function body.");

        let doc = if self.match_token(TokenType::Doc) {
            Some(self.previous)
        } else {
            None
        };

        let body = self.block();

        // Restore previous function type
        self.fn_type = previous_fn_type;
        let mangled_name = self.scopes.join("$");
        self.scopes.pop();

        Some(Stmt::Function(FunctionDecl {
            name,
            mangled_name,
            doc,
            params,
            return_type,
            body,
            fn_type,
            visibility,
            line: name.line,
        }))
    }

    fn const_declaration(&mut self, visibility: Visibility) -> Option<Stmt<'gc>> {
        self.consume(TokenType::Identifier, "Expect constant name.");
        let name = self.previous;

        self.consume(
            TokenType::Equal,
            "Const declarations must have an initializer.",
        );
        let initializer = self.expression()?;

        self.consume(
            TokenType::Semicolon,
            "Expect ';' after constant declaration.",
        );
        Some(Stmt::Const {
            name,
            initializer,
            visibility,
            line: name.line,
        })
    }

    fn var_declaration(&mut self, visibility: Visibility) -> Option<Stmt<'gc>> {
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
        Some(Stmt::Let(VariableDecl {
            name,
            initializer,
            visibility,
            line: name.line,
        }))
    }

    fn while_statement(&mut self) -> Option<Stmt<'gc>> {
        self.consume(TokenType::OpenParen, "Expect '(' after 'while'.");
        let condition = self.expression()?;
        self.consume(TokenType::CloseParen, "Expect ')' after condition.");
        self.loop_depth += 1;
        let body = Box::new(self.statement()?);
        self.loop_depth -= 1;
        Some(Stmt::Loop {
            initializer: None,
            condition,
            body,
            increment: None,
            line: self.previous.line,
        })
    }

    fn for_statement(&mut self) -> Option<Stmt<'gc>> {
        self.consume(TokenType::OpenParen, "Expect '(' after 'for'.");

        let initializer = if self.match_token(TokenType::Semicolon) {
            None
        } else if self.match_token(TokenType::Let) {
            Some(Box::new(self.var_declaration(Visibility::Private)?))
        } else {
            Some(Box::new(self.expression_statement()?))
        };

        let condition = if !self.check(TokenType::Semicolon) {
            self.expression()?
        } else {
            Expr::Literal {
                value: Literal::Boolean(true),
                line: self.previous.line,
            }
        };
        self.consume(TokenType::Semicolon, "Expect ';' after loop condition.");

        let increment = if !self.check(TokenType::CloseParen) {
            Some(self.expression()?)
        } else {
            None
        };
        self.consume(TokenType::CloseParen, "Expect ')' after for clauses.");

        self.loop_depth += 1;
        let body = Box::new(self.statement()?);
        self.loop_depth -= 1;

        Some(Stmt::Loop {
            initializer,
            condition,
            increment,
            body,
            line: self.previous.line,
        })
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
            if self.fn_type == FunctionType::Constructor {
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
            value: Literal::Number(value),
            line: self.previous.line,
        })
    }

    fn string(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let string = self.previous.lexeme.trim_matches('"');
        Some(Expr::Literal {
            value: Literal::String(self.ctx.intern(string.as_bytes())),
            line: self.previous.line,
        })
    }

    fn literal(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        match self.previous.kind {
            TokenType::False => Some(Expr::Literal {
                value: Literal::Boolean(false),
                line: self.previous.line,
            }),
            TokenType::True => Some(Expr::Literal {
                value: Literal::Boolean(true),
                line: self.previous.line,
            }),
            TokenType::Nil => Some(Expr::Literal {
                value: Literal::Nil,
                line: self.previous.line,
            }),
            _ => unreachable!(),
        }
    }

    fn object(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let mut properties = Vec::new();
        let line = self.previous.line;

        // Empty object
        if self.check(TokenType::CloseBrace) {
            self.advance();
            return Some(Expr::Object { properties, line });
        }

        loop {
            // Property key can be either identifier or string
            let key = if self.check(TokenType::String) {
                self.advance();
                // Remove quotes from string literal
                let lexeme = self.previous.lexeme.trim_matches('"');
                Token::new(TokenType::Identifier, lexeme, self.previous.line)
            } else if self.check(TokenType::Identifier) {
                self.advance();
                self.previous
            } else {
                self.error_at_current("Expected property name string or identifier.");
                return None;
            };

            // Parse colon
            self.consume(TokenType::Colon, "Expected ':' after property name.");

            // Parse property value
            let value = self.expression()?;
            properties.push((key, value));

            if !self.match_token(TokenType::Comma) {
                break;
            }

            // Allow trailing comma
            if self.check(TokenType::CloseBrace) {
                break;
            }
        }

        self.consume(TokenType::CloseBrace, "Expected '}' after object literal.");

        Some(Expr::Object { properties, line })
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

    fn argument_list(&mut self) -> Option<(Vec<Expr<'gc>>, HashMap<String, Expr<'gc>>)> {
        let mut arguments = Vec::new();
        let mut keyword_args = HashMap::new();

        if !self.check(TokenType::CloseParen) {
            loop {
                if self.check(TokenType::Identifier)
                    && matches!(self.peek_next(), Some(t) if t.kind == TokenType::Equal)
                {
                    self.advance();
                    let name = self.previous;
                    self.advance(); // consume '='
                    let value = self.expression()?;
                    keyword_args.insert(name.lexeme.to_string(), value);
                } else {
                    if !keyword_args.is_empty() {
                        self.error("Positional arguments must come before keyword arguments.");
                    }
                    arguments.push(self.expression()?);
                }

                if arguments.len() + keyword_args.len() > 255 {
                    self.error("Can't have more than 255 arguments.");
                    break;
                }

                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }

        Some((arguments, keyword_args))
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

        let (arguments, keyword_args) = self.argument_list()?;
        self.consume(TokenType::CloseParen, "Expect ')' after arguments.");

        Some(Expr::Call {
            callee,
            arguments,
            keyword_args,
            line: self.previous.line,
        })
    }

    fn index(&mut self, can_assign: bool) -> Option<Expr<'gc>> {
        let object = Box::new(self.previous_expr.take()?);

        let key = Box::new(self.expression()?);
        self.consume(TokenType::CloseBracket, "Expect ']' after index.");

        let line = self.previous.line;
        let value = if can_assign && self.match_token(TokenType::Equal) {
            Some(Box::new(self.expression()?))
        } else {
            None
        };

        Some(Expr::Index {
            object,
            key,
            value,
            line,
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
            let (arguments, keyword_args) = self.argument_list()?;
            self.consume(TokenType::CloseParen, "Expect ')' after arguments.");

            Some(Expr::Invoke {
                object,
                method: name,
                arguments,
                keyword_args,
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

    fn match_compound_assignment(&mut self) -> Option<TokenType> {
        let kind = match self.current.kind {
            TokenType::PlusEqual => Some(TokenType::Plus),
            TokenType::MinusEqual => Some(TokenType::Minus),
            TokenType::StarEqual => Some(TokenType::Star),
            TokenType::SlashEqual => Some(TokenType::Slash),
            TokenType::PercentEqual => Some(TokenType::Percent),
            _ => None,
        };

        if kind.is_some() {
            self.advance();
        }
        kind
    }

    fn parse_compound_assignment(
        &mut self,
        name: Token<'gc>,
        op_kind: TokenType,
    ) -> Option<Expr<'gc>> {
        let right = Box::new(self.expression()?);
        let operator = Token::new(op_kind, self.previous.lexeme, self.previous.line);

        Some(Expr::Assign {
            name,
            value: Box::new(Expr::Binary {
                left: Box::new(Expr::Variable {
                    name,
                    line: name.line,
                }),
                operator,
                right,
                line: operator.line,
            }),
            line: operator.line,
        })
    }

    fn variable(&mut self, can_assign: bool) -> Option<Expr<'gc>> {
        let name = self.previous;

        if can_assign {
            if let Some(op_kind) = self.match_compound_assignment() {
                return self.parse_compound_assignment(name, op_kind);
            } else if self.match_token(TokenType::Equal) {
                let value = Box::new(self.expression()?);
                return Some(Expr::Assign {
                    name,
                    value,
                    line: self.previous.line,
                });
            }
        }

        Some(Expr::Variable {
            name,
            line: name.line,
        })
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
            let (arguments, keyword_args) = self.argument_list()?;
            self.consume(TokenType::CloseParen, "Expect ')' after arguments.");

            Some(Expr::SuperInvoke {
                method,
                arguments,
                keyword_args,
                line: keyword.line,
            })
        } else {
            Some(Expr::Super {
                method,
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
        if !self.fn_type.is_ai_function() && self.fn_type != FunctionType::Script {
            self.error("Can't prompt outside of ai function or root script.");
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
        if can_assign && !matches!(expr, Expr::Variable { .. }) {
            match self.current.kind {
                TokenType::Equal
                | TokenType::PlusEqual
                | TokenType::MinusEqual
                | TokenType::StarEqual
                | TokenType::SlashEqual
                | TokenType::PercentEqual => {
                    self.error_at_current("Invalid assignment target.");
                    return None;
                }
                _ => {}
            }
        }
        Some(expr)
    }

    // Helper methods
    fn advance(&mut self) {
        self.previous = mem::take(&mut self.current);

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
    Power,      // **
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
        TokenType::OpenBrace => ParseRule::new(Some(Parser::object), None, Precedence::Primary),
        TokenType::OpenParen => {
            ParseRule::new(Some(Parser::grouping), Some(Parser::call), Precedence::Call)
        }
        TokenType::OpenBracket => {
            ParseRule::new(Some(Parser::array), Some(Parser::index), Precedence::Call)
        }
        TokenType::Dot => ParseRule::new(None, Some(Parser::dot), Precedence::Call),
        TokenType::Minus => {
            ParseRule::new(Some(Parser::unary), Some(Parser::binary), Precedence::Term)
        }
        TokenType::Plus => ParseRule::new(None, Some(Parser::binary), Precedence::Term),
        TokenType::Slash => ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
        TokenType::Star => ParseRule::new(None, Some(Parser::binary), Precedence::Factor),
        TokenType::StarStar => ParseRule::new(
            None,
            Some(Parser::binary),
            Precedence::Power, // Higher precedence than multiplication
        ),
        TokenType::Percent => ParseRule::new(
            None,
            Some(Parser::binary),
            Precedence::Factor, // Same precedence as multiply/divide
        ),
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
