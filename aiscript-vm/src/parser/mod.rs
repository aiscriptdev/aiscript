use std::{
    collections::{HashMap, HashSet},
    iter::{self},
    mem,
    ops::{Add, Deref, DerefMut},
};

use indexmap::IndexMap;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{
    ast::{Expr, Literal, ParameterDecl, Program, Stmt},
    lexer::{Scanner, Token, TokenType},
};
use crate::{
    VmError,
    ast::{
        AgentDecl, ClassDecl, ClassFieldDecl, EnumDecl, EnumVariant, ErrorHandler, FStringPart,
        FunctionDecl, MatchArm, MatchPattern, ObjectProperty, VariableDecl, Visibility,
    },
    object::{FunctionType, ListKind},
    ty::{
        ClassField, EnumVariantChecker, FunctionErrorResolver, Type, TypeResolver, ValidationError,
    },
    vm::Context,
};
use aiscript_directive::DirectiveParser;

mod stmt_test;

type ParseFn<'gc> = fn(&mut Parser<'gc>, bool /*can assign*/) -> Option<Expr<'gc>>;

pub struct Parser<'gc> {
    ctx: Context<'gc>,
    scanner: Scanner<'gc>,
    previous_expr: Option<Expr<'gc>>,
    fn_type: FunctionType,
    class_compiler: Option<Box<ClassCompiler>>,
    scopes: Vec<String>,
    // track if we're inside a loop
    loop_depth: usize,
    // the flag is help to avoid brace confict, for example in:
    // - for's increment
    // - if / else if condition
    // - while condition
    // since we omit the paren around,
    // it is hard to handle the '{' conflict without this flag.
    stop_at_brace: bool,
    in_match_arm: bool,
    type_resolver: TypeResolver<'gc>,
    error_resolver: Option<FunctionErrorResolver<'gc>>,
}

#[derive(Default, Debug)]
struct ClassCompiler {
    has_superclass: bool,
    is_enum: bool,
    enclosing: Option<Box<ClassCompiler>>,
    current_method_type: FunctionType,
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

impl<'gc> Parser<'gc> {
    pub fn new(ctx: Context<'gc>, source: &'gc str) -> Self {
        Parser {
            ctx,
            scanner: Scanner::new(source),
            previous_expr: None,
            fn_type: FunctionType::Script,
            class_compiler: None,
            scopes: Vec::new(),
            loop_depth: 0,
            stop_at_brace: false,
            in_match_arm: false,
            type_resolver: TypeResolver::new(),
            error_resolver: None,
        }
    }

    pub fn parse(&mut self) -> Result<Program<'gc>, VmError> {
        self.scopes.push(String::from("script"));
        let mut program = Program::new();
        self.advance();

        while !self.is_at_end() {
            if let Some(stmt) = self.declaration() {
                match &stmt {
                    Stmt::Class(ClassDecl { name, .. }) | Stmt::Enum(EnumDecl { name, .. }) => {
                        self.type_resolver
                            .register_type(name.lexeme, Type::Custom(*name));
                    }
                    _ => {}
                }
                program.statements.push(stmt);
            }
        }

        if !self.had_error {
            let type_resolver = mem::take(&mut self.type_resolver);
            // Validate all type usages
            type_resolver.validate_all_types(|token, err| {
                self.error_at(token, &err);
            });
        }

        if self.had_error {
            Err(VmError::CompileError)
        } else {
            Ok(program)
        }
    }

    fn parse_type(&mut self) -> Token<'gc> {
        if !self.check(TokenType::Identifier) && !self.check(TokenType::Error) {
            self.error_at_current("Invalid type annotation.");
        }
        // Parse either builtin type or custom type (identifier)
        self.advance();
        // Record type usage for later validation
        self.type_resolver.add_type_usage(self.previous);
        self.previous
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
        } else if self.match_token(TokenType::Enum) {
            self.enum_declaration(visibility)
        } else if self.match_token(TokenType::Class) {
            self.class_declaration(visibility)
        } else if self.match_token(TokenType::AI) {
            self.consume(TokenType::Fn, "Expect 'fn' after 'ai'.");
            self.func_declaration(FunctionType::Function { is_ai: true }, visibility)
        } else if self.match_token(TokenType::Fn) {
            self.func_declaration(FunctionType::Function { is_ai: false }, visibility)
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
        if self.match_token(TokenType::OpenBrace) {
            self.block_statement()
        } else if self.match_token(TokenType::If) {
            self.if_statement()
        } else if self.match_token(TokenType::Raise) {
            self.raise_statement()
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
                    if !matches!(value, Expr::List { .. }) {
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

    fn enum_declaration(&mut self, visibility: Visibility) -> Option<Stmt<'gc>> {
        self.consume_either(TokenType::Identifier, TokenType::Error, "Expect enum name.");
        let name = self.previous;
        self.scopes.push(name.lexeme.to_owned());
        if self.check(TokenType::OpenParen) && self.check_next(TokenType::Identifier) {
            self.error_at_current("Enum doesn't support inherit.");
        }

        let class_compiler = ClassCompiler {
            has_superclass: false,
            is_enum: true,
            enclosing: self.class_compiler.take(),
            current_method_type: self.fn_type,
        };
        self.class_compiler = Some(Box::new(class_compiler));
        self.consume(TokenType::OpenBrace, "Expect '{' before enum body.");

        let mut variants = Vec::new();
        let mut methods = Vec::new();
        let mut checker = EnumVariantChecker::new(name.lexeme);

        while !self.check(TokenType::CloseBrace) && !self.is_at_end() {
            if self.current.is_function_def_keyword() {
                // Parse method
                methods.push(self.method_declaration()?);
                continue;
            }

            // Parse variant
            if !self.check(TokenType::Identifier) {
                self.error_at_current("Expect variant name.");
            }
            // Consume the variant identifier
            self.advance();
            let variant_name = self.previous;
            if let Err(err) = checker.check_variant(variant_name) {
                self.error_at(variant_name, &err);
            }

            let value = if self.match_token(TokenType::Equal) {
                // Check for valid literal tokens
                if !self.current.is_literal_token() {
                    self.error_at_current(
                        "Enum variant value must be a literal (number, string, or boolean)",
                    );
                    return None;
                }

                if let Some(Expr::Literal { value: literal, .. }) = self.expression() {
                    if let Err(msg) = checker.check_value(variant_name, &literal) {
                        self.error_at(variant_name, &msg);
                        return None;
                    }
                    Some(literal)
                } else {
                    None
                }
            } else {
                // Auto-increment check
                if !checker.is_auto_increment_supported() {
                    self.error_at(
                        variant_name,
                        "Must specify value for non-integer enum variants",
                    );
                    return None;
                }
                checker.next_value()
            };

            variants.push(EnumVariant {
                name: variant_name,
                value: value.unwrap_or_default(),
            });

            if !self.check(TokenType::CloseBrace) {
                self.consume(TokenType::Comma, "Expect ',' after variant.");
            }
        }

        self.consume(TokenType::CloseBrace, "Expect '}' after enum body.");

        self.scopes.pop();
        // pop that compiler off the stack and restore the enclosing class compiler.
        self.class_compiler = self.class_compiler.take().and_then(|c| c.enclosing);
        Some(Stmt::Enum(EnumDecl {
            name,
            variants,
            methods,
            visibility,
            line: name.line,
        }))
    }

    fn enum_variant(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        // The enum name is in previous_expr since this is an infix operator
        let enum_name = match &self.previous_expr.take()? {
            Expr::Variable { name, .. } => *name,
            _ => {
                self.error("Expected enum name before '::'.");
                return None;
            }
        };

        self.consume(TokenType::Identifier, "Expect variant name after '::'.");
        let variant = self.previous;

        Some(Expr::EnumVariant {
            enum_name,
            variant,
            line: variant.line,
        })
    }

    fn class_declaration(&mut self, visibility: Visibility) -> Option<Stmt<'gc>> {
        self.consume_either(
            TokenType::Identifier,
            TokenType::Error,
            "Expect class name.",
        );
        let name = self.previous;
        if name.is_error_type() && self.match_token(TokenType::OpenParen) {
            self.error_at(name, "Error types cannot inherit from other classes.");
            return None;
        }

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
            is_enum: false,
            enclosing: self.class_compiler.take(),
            current_method_type: self.fn_type,
        };
        self.class_compiler = Some(Box::new(class_compiler));

        self.type_resolver.register_class(name);
        self.consume(TokenType::OpenBrace, "Expect '{' before class body.");

        let mut fields = Vec::new();
        let mut methods = Vec::new();
        while !self.check(TokenType::CloseBrace) && !self.is_at_end() {
            let mut validators = Vec::new();
            if self.check(TokenType::At) {
                validators = DirectiveParser::new(&mut self.scanner).parse_validators();
            }
            if self.check(TokenType::Identifier) && !self.check_next(TokenType::OpenParen) {
                let mut field = self.parse_class_field()?;
                self.type_resolver.add_class_field(
                    name.lexeme,
                    ClassField {
                        name: field.name,
                        ty: Type::from_token(field.type_hint),
                        required: field.default_value.is_none(),
                    },
                );
                field.validators = validators;
                fields.push(field);
            } else {
                methods.push(self.method_declaration()?);
            }
        }

        fn is_self_field_init<'gc>(stmt: &Stmt<'gc>) -> Option<&'gc str> {
            if let Stmt::Expression {
                expression: Expr::Set { object, name, .. },
                ..
            } = stmt
            {
                // Only count assignments to self.field
                if matches!(**object, Expr::Self_ { .. }) {
                    return Some(name.lexeme);
                }
            }
            None
        }

        fn create_field_init(field: ClassFieldDecl) -> Stmt {
            Stmt::Expression {
                expression: Expr::Set {
                    object: Box::new(Expr::Self_ { line: field.line }),
                    name: field.name,
                    value: Box::new(field.default_value.unwrap_or_else(|| Expr::Literal {
                        value: Literal::Nil,
                        line: field.line,
                    })),
                    line: field.line,
                },
                line: field.line,
            }
        }

        // Get the single constructor if it exists
        if !fields.is_empty() {
            let (constructor, other_methods): (Vec<_>, Vec<_>) = methods.into_iter()
            .partition(
                |m| matches!(m, Stmt::Function(FunctionDecl { fn_type, .. }) if fn_type.is_constructor()),
            );
            methods = other_methods;
            let constructor = if let Some(Stmt::Function(mut constructor_decl)) =
                constructor.into_iter().next()
            {
                // Track which fields are initialized in constructor through self.field = ...
                let initialized_fields: HashSet<_> = constructor_decl
                    .body
                    .iter()
                    .filter_map(is_self_field_init)
                    .collect();

                // Create initialization statements for declared fields that aren't initialized
                let field_inits = fields
                    .into_iter()
                    .filter(|field| !initialized_fields.contains(field.name.lexeme))
                    .map(create_field_init)
                    .collect::<Vec<_>>();

                if !field_inits.is_empty() {
                    let mut new_body = field_inits;
                    // Keep all original statements, including assignments to non-declared fields
                    new_body.extend(mem::take(&mut constructor_decl.body));
                    // TODO: add validators for parameters
                    Stmt::Function(FunctionDecl {
                        body: new_body,
                        ..constructor_decl
                    })
                } else {
                    // No declared fields need initialization, use original constructor as-is
                    Stmt::Function(constructor_decl)
                }
            } else {
                // No constructor exists, create synthetic one that declare all fields as keyword arguments:
                /*
                    class Foo {
                        x: int = 0,
                        y: int = 0,

                        // Auto-generated constructor
                        fn new(x: int = 0, y: int = 0) {
                            self.x = x;
                            self.y = y;
                        }
                    }
                */
                let mut params = IndexMap::with_capacity(fields.len());
                let mut body = Vec::with_capacity(fields.len());
                for field in fields {
                    // Note: the line is not real, we just give the field's line.
                    let line = field.line;
                    body.push(Stmt::Expression {
                        expression: Expr::Set {
                            object: Box::new(Expr::Self_ { line }),
                            name: field.name,
                            value: Box::new(Expr::Variable {
                                name: field.name,
                                line,
                            }),
                            line,
                        },
                        line,
                    });
                    params.insert(
                        field.name,
                        ParameterDecl {
                            name: field.name,
                            type_hint: Some(field.type_hint),
                            default_value: field.default_value,
                            validators: field.validators,
                        },
                    );
                }
                Stmt::Function(FunctionDecl {
                    name: Token::new(TokenType::Identifier, "new", name.line),
                    mangled_name: format!("{}$new", self.scopes.join("$")),
                    params,
                    doc: None,
                    return_type: None,
                    error_types: Vec::new(),
                    body,
                    fn_type: FunctionType::Constructor,
                    visibility: Visibility::Public,
                    line: name.line,
                })
            };
            methods.push(constructor);
        }

        self.consume(TokenType::CloseBrace, "Expect '}' after class body.");

        self.scopes.pop();
        // pop that compiler off the stack and restore the enclosing class compiler.
        self.class_compiler = self.class_compiler.take().and_then(|c| c.enclosing);
        Some(Stmt::Class(ClassDecl {
            name,
            superclass,
            // fields,
            methods,
            visibility,
            line: name.line,
        }))
    }

    fn parse_class_field(&mut self) -> Option<ClassFieldDecl<'gc>> {
        self.consume(TokenType::Identifier, "Expect field name.");
        let name = self.previous;

        self.consume(TokenType::Colon, "Expect ':' after field name.");
        let type_hint = self.parse_type();

        let default_value = if self.match_token(TokenType::Equal) {
            if self.current.is_literal_token() {
                Some(self.expression()?)
            } else if self.check(TokenType::Identifier) {
                self.error_at_current(
                    "Only allow set literal (number, string, bool) as the default value.",
                );
                None
            } else {
                self.error_at_current("Expect default value after '='.");
                None
            }
        } else {
            None
        };

        self.consume(TokenType::Comma, "Expect ',' after field declaration.");

        Some(ClassFieldDecl {
            name,
            type_hint,
            default_value,
            validators: Vec::new(),
            line: name.line,
        })
    }

    fn method_declaration(&mut self) -> Option<Stmt<'gc>> {
        let method_vis = if self.match_token(TokenType::Pub) {
            Visibility::Public
        } else {
            Visibility::Private
        };
        let method = if self.match_token(TokenType::AI) {
            self.consume(TokenType::Fn, "Expect 'fn' after 'ai'.");
            self.func_declaration(
                FunctionType::Method {
                    is_ai: true,
                    is_static: false,
                },
                method_vis,
            )?
        } else if self.match_token(TokenType::Fn) {
            self.func_declaration(
                FunctionType::Method {
                    is_ai: false,
                    is_static: false,
                },
                method_vis,
            )?
        } else {
            self.error_at_current("Expect 'fn' or 'ai fn' modifier for method.");
            return None;
        };
        Some(method)
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
            FunctionType::Method { .. } => "method",
            FunctionType::Tool => "tool function",
            _ => "function",
        };

        self.consume(TokenType::Identifier, &format!("Expect {type_name} name."));
        let name = self.previous;
        self.scopes.push(name.lexeme.to_string());
        if self.fn_type.is_method() && name.lexeme == "new" {
            self.fn_type = FunctionType::Constructor;
        }
        // Store the previous function info and create new one for this function
        let previous_error_resolver = self.error_resolver.take();
        self.error_resolver = Some(FunctionErrorResolver::new(name));

        self.consume(TokenType::OpenParen, "Expect '(' after function name.");

        // Use IndexMap for parameters and their types
        // IndexMap is ordered by insertion order,
        // which is matter for function call
        let mut params = IndexMap::new();
        let mut keyword_args_count = 0;
        let mut self_args_count = 0;
        loop {
            if self.check(TokenType::CloseParen) {
                break;
            }
            if params.len() >= 255 {
                self.error_at_current("Can't have more than 255 parameters.");
            }

            if self.check(TokenType::Self_) {
                self.advance();
                match self.fn_type {
                    FunctionType::Method { .. } if (self_args_count > 0 || !params.is_empty()) => {
                        self.error("'self' only allow as the first paramater.");
                    }
                    FunctionType::Function { .. } | FunctionType::Tool => {
                        self.error("'self' parameter is only allowed in class methods");
                    }
                    FunctionType::Constructor => {
                        self.error("No need to declare 'self' parameter for class constructor.");
                    }
                    _ => {
                        // unreachable
                    }
                }

                self_args_count += 1;
                if self.check(TokenType::Comma) {
                    self.advance();
                } else if self.check_next(TokenType::CloseParen) {
                    self.consume(TokenType::Comma, "Expect ',' between 'self' and parameter.");
                }
                continue;
            }

            if self.check(TokenType::Super) {
                self.advance();
                self.error("Can't use 'super' as function paramter.");
            } else {
                self.consume(TokenType::Identifier, "Expect parameter name.");
            }
            let param_name = self.previous;

            // Parse parameter type annotation
            let type_hint = if self.match_token(TokenType::Colon) {
                Some(self.parse_type())
            } else {
                None
            };

            // Parse default value if present - must be a literal
            let default_value = if self.match_token(TokenType::Equal) {
                match self.expression() {
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
                ParameterDecl {
                    name: param_name,
                    type_hint,
                    default_value,
                    validators: Vec::new(), // TODO: support validator for normal function?
                },
            );

            if !self.match_token(TokenType::Comma) {
                break;
            }
        }
        self.consume(TokenType::CloseParen, "Expect ')' after parameters.");

        if self_args_count == 0 {
            // Set this method to static
            if let FunctionType::Method { is_ai, .. } = self.fn_type {
                self.fn_type = FunctionType::Method {
                    is_ai,
                    is_static: true,
                };
            }
        }

        if self.fn_type.is_method() {
            // Update class compiler's current method type
            if let Some(class_compiler) = self.class_compiler.as_mut() {
                class_compiler.current_method_type = self.fn_type;
            }
        }

        // Parse optional return type and error types
        let (return_type, error_types) = self.parse_function_return();
        self.consume(TokenType::OpenBrace, "Expect '{' before function body.");

        let doc = if self.match_token(TokenType::Doc) {
            Some(self.previous)
        } else {
            None
        };

        let body = self.block_expr();

        let mangled_name = self.scopes.join("$");
        self.scopes.pop();

        // if let Some(resolver) = &self.error_resolver {
        //     if let Err(err) = resolver.validate() {
        //         self.error_at(name, &err);
        //     }
        // }
        // Restore the previous function context
        self.error_resolver = previous_error_resolver;

        let func = Stmt::Function(FunctionDecl {
            name,
            mangled_name,
            doc,
            params,
            return_type,
            error_types,
            body,
            fn_type: self.fn_type,
            visibility,
            line: name.line,
        });
        // Restore previous function type
        self.fn_type = previous_fn_type;
        Some(func)
    }

    // Helper method to check if we're inside a function that can handle errors
    fn in_error_function(&self) -> bool {
        self.error_resolver
            .as_ref()
            .map(|f| f.in_error_function)
            .unwrap_or(false)
    }

    fn parse_function_return(&mut self) -> (Option<Token<'gc>>, Vec<Token<'gc>>) {
        let mut return_type = None;
        let mut error_types = Vec::new();

        if self.match_token(TokenType::Arrow) {
            if !self.check_either(TokenType::Error, TokenType::Identifier) {
                self.error_at_current("Expect type after '->'.");
                return (None, Vec::new());
            }
            let first_type = self.parse_type();

            // Check if first type is an error type
            if first_type.is_error_type() {
                if let Some(resolver) = self.error_resolver.as_mut() {
                    resolver.add_declared_error(first_type);
                }
                error_types.push(first_type);
            } else {
                return_type = Some(first_type);
            }

            // Parse additional types (must be error types)
            loop {
                if self.check(TokenType::OpenBrace) {
                    break;
                }
                // Handle separator
                if self.check(TokenType::Comma) {
                    self.error_at_current(
                        "Expected '|' to separate return type and error types, found ','.",
                    );
                    self.advance(); // Consume the comma
                } else if self.check(TokenType::Pipe) {
                    self.advance(); // Consume the pipe
                } else {
                    // If we see anything else without a separator, that's an error
                    if !self.check(TokenType::OpenBrace) {
                        self.error_at_current("Expected '|' before error type.");
                    }
                }

                if self.match_token(TokenType::Error) {
                    let error_type = self.previous;
                    if let Some(resolver) = self.error_resolver.as_mut() {
                        resolver.add_declared_error(error_type);
                    }
                    error_types.push(error_type);
                } else {
                    self.error_at_current("Only error types can be listed after return type.");
                    self.advance();
                }
            }
        }

        (return_type, error_types)
    }

    fn lambda(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let line = self.previous.line;
        let previous_fn_type = self.fn_type;
        self.fn_type = FunctionType::Lambda;
        let mut params = Vec::new();

        // Parse parameters between pipes (||)
        if !self.match_token(TokenType::Pipe) {
            loop {
                if params.len() >= 255 {
                    self.error_at_current("Can't have more than 255 parameters.");
                    break;
                }

                self.consume(TokenType::Identifier, "Expect parameter name.");
                params.push(self.previous);

                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
            self.consume(TokenType::Pipe, "Expect '|' after lambda parameters.");
        }

        let body = if self.match_token(TokenType::OpenBrace) {
            let statements = self.block_expr();
            let next_token = self.current;
            if self.parse_error_handling().is_some() {
                self.error_at(next_token, "Lambda doesn't support declare error handler.");
            }
            Box::new(Expr::Block {
                statements,
                line: self.previous.line,
            })
        } else {
            // Single expression body - wrap in a block with return statement
            let expr = self.expression()?;
            Box::new(Expr::Block {
                statements: vec![Stmt::Return {
                    value: Some(expr),
                    line: self.previous.line,
                }],
                line: self.previous.line,
            })
        };

        self.fn_type = previous_fn_type;
        Some(Expr::Lambda { params, body, line })
    }

    fn pipe_arrow(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        // Save the left side of the pipe
        let left = Box::new(self.previous_expr.take()?);

        // Parse the function name as an identifier
        self.consume(TokenType::Identifier, "Expect function name after |>.");
        let callee_name = self.previous;
        let callee = Box::new(Expr::Variable {
            name: callee_name,
            line: callee_name.line,
        });

        let mut arguments = Vec::new();
        let mut keyword_args = HashMap::new();

        // Check if we have explicit parentheses
        if self.match_token(TokenType::OpenParen) {
            // Parse arguments if any
            if !self.check(TokenType::CloseParen) {
                let (args, kw_args) = self.argument_list()?;
                arguments = args;
                keyword_args = kw_args;
            }
            self.consume(TokenType::CloseParen, "Expect ')' after arguments.");
        }

        // Create call expression with left being first argument
        Some(Expr::Call {
            callee,
            is_constructor: false,
            arguments: std::iter::once(*left).chain(arguments).collect(),
            keyword_args,
            error_handler: self.parse_error_handling(),
            line: callee_name.line,
        })
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
        // Set flag before parsing condition
        self.stop_at_brace = true;
        let condition = self.expression()?;
        self.stop_at_brace = false;

        self.consume(TokenType::OpenBrace, "Expect '{' before loop body.");
        self.loop_depth += 1;
        let body = Box::new(self.block_statement()?);
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

        // Parse increment - optional
        let increment = if !self.check(TokenType::OpenBrace) {
            self.stop_at_brace = true;
            let expr = self.parse_precedence(Precedence::Assignment)?;
            self.stop_at_brace = false;
            Some(expr)
        } else {
            // Peek ahead to check for empty object literal
            if self.check(TokenType::OpenBrace)
                && matches!(self.peek_next(), Some(t) if t.kind == TokenType::CloseBrace)
            {
                self.error_at_current("Empty object literal not allowed in for loop increment");
                return None;
            }
            None
        };

        self.consume(TokenType::OpenBrace, "Expect '{' before loop body.");
        self.loop_depth += 1;
        let body = Box::new(self.block_statement()?);
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
        // Set the flag before parsing condition
        self.stop_at_brace = true;
        let condition = self.expression()?;
        // Clear the flag after parsing condition
        self.stop_at_brace = false;

        self.consume(TokenType::OpenBrace, "Expect '{' before then branch.");
        let then_branch = Box::new(self.block_statement()?);

        let else_branch = if self.match_token(TokenType::Else) {
            if self.match_token(TokenType::If) {
                Some(Box::new(self.if_statement()?))
            } else {
                self.consume(TokenType::OpenBrace, "Expect '{' before else branch.");
                Some(Box::new(self.block_statement()?))
            }
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

    fn block_statement(&mut self) -> Option<Stmt<'gc>> {
        let statements = self.block();
        Some(Stmt::Block {
            statements,
            line: self.previous.line,
        })
    }

    fn return_statement(&mut self) -> Option<Stmt<'gc>> {
        let value = if !self.check(TokenType::Semicolon) {
            if self.fn_type == FunctionType::Constructor {
                self.error("Can't return a value from an constructor.");
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

    // Parse block expression. The different between block_expr() and block() are:
    // - block_expr() will treat the last line as an expression if it not ends with semicolon.
    // - block_expr() nornamlly wrapped in Expr::Block while block() wraped in Stmt::Block
    fn block_expr(&mut self) -> Vec<Stmt<'gc>> {
        let mut statements = Vec::new();

        while !self.check(TokenType::CloseBrace) && !self.is_at_end() {
            // Check if we're looking at a potential expression
            if self.current.is_expr_start() {
                // Parse as expression
                if let Some(expr) = self.expression() {
                    if self.check(TokenType::CloseBrace) {
                        // It's a tail expression - create special BlockReturn statement
                        // This is different from Return in that it only returns from the block
                        statements.push(Stmt::BlockReturn {
                            value: expr,
                            line: self.previous.line,
                        });
                        break;
                    } else {
                        // Not a tail expression, must have semicolon
                        if self.check(TokenType::Semicolon) {
                            self.advance();
                        } else {
                            self.error("Expect ';' after expression.");
                        }

                        statements.push(Stmt::Expression {
                            expression: expr,
                            line: self.previous.line,
                        });
                    }
                }
            } else if let Some(declaration) = self.declaration() {
                statements.push(declaration);
            }
        }

        self.consume(TokenType::CloseBrace, "Expect '}' after block.");
        statements
    }

    fn expression(&mut self) -> Option<Expr<'gc>> {
        self.parse_precedence(Precedence::Assignment)
    }

    fn inline_if(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let then_expr = Box::new(self.previous_expr.take()?);
        let condition = Box::new(self.expression()?);

        self.consume(TokenType::Else, "Expect 'else' after inline if condition.");
        let else_expr = Box::new(self.expression()?);

        Some(Expr::InlineIf {
            condition,
            then_branch: then_expr,
            else_branch: else_expr,
            line: self.previous.line,
        })
    }

    fn number(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let value = self.previous.lexeme.parse::<f64>().unwrap();
        Some(Expr::Literal {
            value: Literal::Number(value),
            line: self.previous.line,
        })
    }

    fn string(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let lexeme = self.previous.lexeme;
        let escaped_string = self.escape_string(lexeme)?;
        Some(Expr::Literal {
            value: Literal::String(self.ctx.intern(escaped_string.as_bytes())),
            line: self.previous.line,
        })
    }

    fn fstring(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let line = self.previous.line;
        let content = self.previous.lexeme;

        // Parse the f-string content to extract string literals and expressions
        let parts = self.parse_fstring_content(content)?;
        Some(Expr::FString { parts, line })
    }

    fn parse_fstring_content(&mut self, content: &'gc str) -> Option<Vec<FStringPart<'gc>>> {
        let mut parts = Vec::new();
        let mut in_expression = false;
        let mut brace_depth = 0;
        let mut start = 0;

        // Iterate through the string content character by character
        let chars: Vec<(usize, char)> = content.char_indices().collect();

        for (i, c) in chars.iter() {
            match c {
                '{' => {
                    if *i > 0 && chars[i - 1].1 == '\\' {
                        // Escaped brace, skip
                        continue;
                    }

                    if brace_depth == 0 {
                        // Start of an expression
                        if start < *i {
                            // Add the preceding string literal part
                            let str_part = &content[start..*i];
                            let escaped_str = self.escape_string(str_part)?;
                            parts.push(FStringPart::StringLiteral(
                                self.ctx.intern(escaped_str.as_bytes()),
                            ));
                        }
                        in_expression = true;
                        start = *i + 1; // Start after the opening brace
                    }
                    brace_depth += 1;
                }
                '}' => {
                    if *i > 0 && chars[i - 1].1 == '\\' {
                        // Escaped brace, skip
                        continue;
                    }

                    if in_expression && brace_depth == 1 {
                        // End of an expression
                        let expr_str = &content[start..*i];
                        if !expr_str.is_empty() {
                            // Parse the expression using a temporary parser
                            let mut temp_parser = Parser::new(self.ctx, expr_str);
                            temp_parser.advance(); // Initialize the first token

                            match temp_parser.expression() {
                                Some(expr) => {
                                    parts.push(FStringPart::Expression(Box::new(expr)));
                                }
                                None => {
                                    self.error(&format!(
                                        "Invalid expression in f-string: '{}'",
                                        expr_str
                                    ));
                                    return None;
                                }
                            }
                        } else {
                            self.error("Empty expression in f-string");
                            return None;
                        }

                        in_expression = false;
                        start = *i + 1; // Start after the closing brace
                    }

                    if brace_depth > 0 {
                        brace_depth -= 1;
                    } else {
                        self.error("Unmatched closing brace in f-string");
                        return None;
                    }
                }
                _ => {
                    // Just continue for all other characters
                }
            }
        }

        // Add the final string part if there is one
        if !in_expression && start < content.len() {
            let str_part = &content[start..];
            let escaped_str = self.escape_string(str_part)?;
            parts.push(FStringPart::StringLiteral(
                self.ctx.intern(escaped_str.as_bytes()),
            ));
        }

        if in_expression {
            self.error("Unterminated expression in f-string");
            return None;
        }

        Some(parts)
    }

    fn raw_string(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        Some(Expr::Literal {
            value: Literal::String(self.ctx.intern(self.previous.lexeme.as_bytes())),
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
        if self.stop_at_brace {
            self.error("Cannot use object literals in flow control conditions");
            return None;
        }

        let line = self.previous.line;
        let previous_expr = self.previous_expr.take();
        let mut properties = Vec::new();

        if !self.check(TokenType::CloseBrace) {
            loop {
                let property = if self.match_token(TokenType::OpenBracket) {
                    // Computed property name: [expr]
                    let key_expr = Box::new(self.expression()?);
                    self.consume(
                        TokenType::CloseBracket,
                        "Expect ']' after computed property name.",
                    );
                    self.consume(TokenType::Colon, "Expect ':' after computed property name.");
                    let value = Box::new(self.expression()?);

                    ObjectProperty::Computed { key_expr, value }
                } else if self.check(TokenType::String) {
                    // String literal key
                    self.advance();
                    let key = Token::new(
                        TokenType::Identifier,
                        self.previous.lexeme,
                        self.previous.line,
                    );
                    self.consume(TokenType::Colon, "Expect ':' after property name.");
                    let value = Box::new(self.expression()?);

                    ObjectProperty::Literal { key, value }
                } else if self.check(TokenType::Identifier) {
                    // Could be either shorthand {a} or regular {a: expr}
                    self.advance();
                    let key = self.previous;

                    if self.match_token(TokenType::Colon) {
                        // Regular property
                        let value = Box::new(self.expression()?);
                        ObjectProperty::Literal { key, value }
                    } else {
                        // Shorthand property {a} -> {a: a}
                        let value = Box::new(Expr::Variable {
                            name: key,
                            line: key.line,
                        });
                        ObjectProperty::Literal { key, value }
                    }
                } else {
                    self.error_at_current(
                        "Expect property name string, identifier, or computed [expression].",
                    );
                    return None;
                };

                properties.push(property);

                if !self.match_token(TokenType::Comma) {
                    break;
                }

                // Allow trailing comma
                if self.check(TokenType::CloseBrace) {
                    break;
                }
            }
        }

        self.consume(TokenType::CloseBrace, "Expect '}' after object literal.");

        if let Some(Expr::Variable { name, .. }) = previous_expr {
            // Validate class initialization
            if let Err(errors) = self
                .type_resolver
                .validate_object_literal(name, &properties)
            {
                // Report each error with its own line number
                for error in errors {
                    match error {
                        ValidationError::DuplicateField(class_token, field_token) => {
                            self.error_at(
                                class_token,
                                &format!("Duplicate field '{}'", field_token.lexeme),
                            );
                        }
                        ValidationError::ClassNotFound(token) => {
                            self.error_at(token, &format!("Class '{}' not found", token.lexeme));
                        }
                        ValidationError::MissingFields(token, fields) => {
                            self.error_at(
                                token,
                                &format!("Missing required fields: {}", fields.join(", ")),
                            );
                        }
                        ValidationError::InvalidField(class_token, field_token) => {
                            self.error_at(
                                class_token,
                                &format!("Invalid field '{}'", field_token.lexeme),
                            );
                        }
                        ValidationError::TypeError {
                            class_token,
                            field_token,
                            expected_type,
                        } => {
                            self.error_at(
                                class_token,
                                &format!(
                                    "Field '{}': Type mismatch: expected {}",
                                    field_token.lexeme,
                                    expected_type.type_name()
                                ),
                            );
                        }
                        ValidationError::ComputedPropertyError(token) => {
                            self.error_at(
                                token,
                                "Computed properties not allowed in class initialization",
                            );
                        }
                    }
                }
                return None;
            }
            // Convert to constructor call
            let mut keyword_args = HashMap::new();
            for prop in properties {
                match prop {
                    ObjectProperty::Literal { key, value } => {
                        keyword_args.insert(key.lexeme.to_string(), *value);
                    }
                    ObjectProperty::Computed { .. } => {
                        self.error("Computed properties not allowed in class initialization");
                        return None;
                    }
                }
            }
            Some(Expr::Call {
                callee: Box::new(previous_expr.unwrap()),
                is_constructor: true,
                arguments: vec![],
                keyword_args,
                error_handler: self.parse_error_handling(),
                line,
            })
        } else {
            Some(Expr::Object { properties, line })
        }
    }

    fn grouping(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        if self.check(TokenType::CloseParen) {
            // Empty tuple
            self.advance();
            return Some(Expr::List {
                kind: ListKind::Tuple,
                elements: Vec::new(),
                line: self.previous.line,
            });
        }

        // Parse the first expression
        let expr = self.expression()?;

        if self.match_token(TokenType::Comma) {
            // It's a tuple with at least one element
            let mut elements = vec![expr];

            // Parse remaining elements if any
            while !self.check(TokenType::CloseParen) && !self.is_at_end() {
                if self.match_token(TokenType::Comma) && self.check(TokenType::CloseParen) {
                    break; // Allow trailing comma
                }

                elements.push(self.expression()?);

                if !self.check(TokenType::CloseParen) {
                    self.consume(TokenType::Comma, "Expect ',' after tuple element.");
                }
            }

            self.consume(TokenType::CloseParen, "Expect ')' after tuple elements.");

            Some(Expr::List {
                kind: ListKind::Tuple,
                elements,
                line: self.previous.line,
            })
        } else {
            // Just a parenthesized expression
            self.consume(TokenType::CloseParen, "Expect ')' after expression.");
            Some(Expr::Grouping {
                expression: Box::new(expr),
                line: self.previous.line,
            })
        }
    }

    fn env_lookup(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let line = self.previous.line;

        let expr = if self.match_token(TokenType::OpenParen) {
            // Handle $(expr) case
            let expr = self.expression()?;
            self.consume(TokenType::CloseParen, "Expect ')' after expression.");
            expr
        } else {
            // Handle $IDENTIFIER case
            self.consume(
                TokenType::Identifier,
                "Expect environment variable name after '$'.",
            );
            // Convert identifier to string literal
            Expr::Literal {
                value: Literal::String(self.ctx.intern(self.previous.lexeme.as_bytes())),
                line: self.previous.line,
            }
        };

        Some(Expr::EnvLookup {
            expr: Box::new(expr),
            line,
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

    fn bracket(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let mut elements = Vec::new();
        let line = self.previous.line;

        // A flag to only check CloseBucket once to determine the Expr::EvaluateVariant
        let mut once = iter::once(1);
        if !self.check(TokenType::CloseBracket) {
            loop {
                let item = self.expression()?;
                if once.next().is_some()
                    // EvaluateVariant can only perform on those three Expr.
                    // We allow [self] syntax in enum's method.
                    && matches!(item, Expr::EnumVariant { .. } | Expr::Variable { .. } | Expr::Self_ { .. })
                    && self.match_token(TokenType::CloseBracket)
                {
                    // Evaluate expression value
                    return Some(Expr::EvaluateVariant {
                        expr: Box::new(item),
                        line,
                    });
                }

                elements.push(item);
                if !self.check(TokenType::Comma) && !self.check(TokenType::CloseBracket) {
                    self.error_at_current("Expect ',' after array element.");
                    return None;
                }

                if !self.match_token(TokenType::Comma) {
                    break;
                }

                // Check for trailing comma
                if self.check(TokenType::CloseBracket) {
                    break;
                }
            }
        }

        self.consume(TokenType::CloseBracket, "Expect ']' after array elements.");
        Some(Expr::List {
            elements,
            kind: ListKind::Array,
            line,
        })
    }

    fn call(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let callee = Box::new(self.previous_expr.take()?);

        let (arguments, keyword_args) = self.argument_list()?;
        self.consume(TokenType::CloseParen, "Expect ')' after arguments.");

        let is_constructor =
            matches!(&*callee, Expr::Variable { name, .. } if self.type_resolver.check_class(name));
        Some(Expr::Call {
            callee,
            is_constructor,
            arguments,
            keyword_args,
            error_handler: self.parse_error_handling(),
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
                error_handler: self.parse_error_handling(),
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

    // Parse error handling after a call/invoke
    fn parse_error_handling(&mut self) -> Option<ErrorHandler<'gc>> {
        let mut handler = None;
        if self.match_token(TokenType::Pipe) {
            self.consume(
                TokenType::Identifier,
                "Expect error variable name after '|'.",
            );
            let error_var = self.previous;

            self.consume(TokenType::Pipe, "Expect closing '|' after error variable.");
            self.consume(
                TokenType::OpenBrace,
                "Expect '{' before error handler body.",
            );

            let handler_body = self.block_expr();

            handler = Some(ErrorHandler {
                error_var,
                handler_body,
                propagate: false,
            });
        } else if self.match_token(TokenType::Question) {
            // Validate ? operator usage
            if !self.in_error_function() {
                self.error("Cannot use '?' operator in function that doesn't declare error types.");
            } else {
                // If we saw a ? but no handler, create an implicit propagation handler
                handler = Some(ErrorHandler {
                    error_var: Token::default(), // Dummy token since we don't need a variable name
                    handler_body: Vec::new(),    // Empty body since we're just propagating
                    propagate: true,
                });
            }
        }

        handler
    }

    fn raise_statement(&mut self) -> Option<Stmt<'gc>> {
        if self.fn_type.is_lambda() {
            self.error("Cannot raise error in lambda.");
            return None;
        }
        if !self.in_error_function() {
            self.error("Cannot use 'raise' outside of a function that declares error types.");
            return None;
        }

        // TODO: check raise error type
        let error = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after raise expression.");
        Some(Stmt::Raise {
            error,
            line: self.previous.line,
        })
    }

    fn match_expression(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let line = self.previous.line;
        self.stop_at_brace = true;
        let expr = Box::new(self.expression()?);
        self.stop_at_brace = false;

        self.consume(TokenType::OpenBrace, "Expect '{' after match expression.");

        if self.check(TokenType::CloseBrace) {
            self.error_at_current(
                "Empty match not allowed. A match expression must have at least one arm.",
            );
            return None;
        }

        let mut arms: Vec<MatchArm> = Vec::new();
        let mut seen_patterns = HashSet::new();
        let mut has_wildcard = false;

        // Set mode for parsing match arms
        self.in_match_arm = true;
        while !self.check(TokenType::CloseBrace) && !self.is_at_end() {
            let arm = self.match_arm()?;

            for pattern in &arm.patterns {
                match pattern {
                    MatchPattern::Wildcard => {
                        if has_wildcard {
                            self.error("Multiple wildcard patterns in match expression.");
                        }
                        has_wildcard = true;
                    }
                    MatchPattern::EnumVariant { enum_name, variant } => {
                        let pattern_key = format!("{}::{}", enum_name.lexeme, variant.lexeme);
                        if !seen_patterns.insert(pattern_key) {
                            self.error_at(*variant, "Duplicate match pattern.");
                        }
                    }
                    MatchPattern::Literal { value } => {
                        if !seen_patterns.insert(format!("{:?}", value)) {
                            self.error("Duplicate match pattern.");
                        }
                    }
                    MatchPattern::Variable { name } => {
                        // FIXME: incorrect duplicate pattern checking due guard hash
                        if !seen_patterns.insert(format!("{} {:?}", name.lexeme, arm.guard)) {
                            self.error("Duplicate match pattern.");
                        }
                    }
                    MatchPattern::Range { .. } => {
                        // Range patterns are validated during parsing
                    }
                }
            }
            arms.push(arm);
        }
        self.in_match_arm = false;

        self.consume(TokenType::CloseBrace, "Expect '}' after match arms.");

        Some(Expr::Match { expr, arms, line })
    }

    fn match_arm(&mut self) -> Option<MatchArm<'gc>> {
        let mut patterns = Vec::new();
        patterns.push(self.match_pattern()?);

        while self.match_token(TokenType::Pipe) {
            patterns.push(self.match_pattern()?);
        }

        // Parse optional guard
        let guard = if self.match_token(TokenType::If) {
            Some(Box::new(self.expression()?))
        } else {
            None
        };

        self.consume(TokenType::FatArrow, "Expect '=>' after match pattern.");

        let body = if self.match_token(TokenType::OpenBrace) {
            let statements = self.block_expr();
            Box::new(Expr::Block {
                statements,
                line: self.previous.line,
            })
        } else {
            Box::new(self.expression()?)
        };

        if !self.check(TokenType::CloseBrace) {
            self.consume(TokenType::Comma, "Expect ',' after match arm.");
        }

        Some(MatchArm {
            patterns,
            guard,
            body,
            line: self.previous.line,
        })
    }

    fn match_pattern(&mut self) -> Option<MatchPattern<'gc>> {
        if self.match_token(TokenType::Underscore) {
            return Some(MatchPattern::Wildcard);
        }

        let current_kind = self.current.kind;
        let start = match current_kind {
            TokenType::Number | TokenType::True | TokenType::False => {
                self.advance();
                if self.check(TokenType::DotDot) || self.check(TokenType::DotDotEq) {
                    let start_expr = Box::new(Expr::Literal {
                        value: self.parse_literal(self.previous)?,
                        line: self.previous.line,
                    });
                    self.parse_range_pattern(Some(start_expr))?
                } else {
                    MatchPattern::Literal {
                        value: self.parse_literal(self.previous)?,
                    }
                }
            }
            TokenType::String => {
                self.advance();
                MatchPattern::Literal {
                    value: Literal::String(self.ctx.intern(self.previous.lexeme.as_bytes())),
                }
            }
            TokenType::Identifier | TokenType::Error => {
                if self.peek_next().map(|t| t.kind) == Some(TokenType::ColonColon) {
                    self.advance(); // consume enum name
                    let enum_name = self.previous;
                    self.advance(); // consume ::
                    self.consume(TokenType::Identifier, "Expect variant name after '::'.");
                    let variant = self.previous;

                    MatchPattern::EnumVariant { enum_name, variant }
                } else {
                    // Variable binding pattern
                    self.advance(); // consume identifier
                    MatchPattern::Variable {
                        name: self.previous,
                    }
                }
            }
            TokenType::DotDot | TokenType::DotDotEq => self.parse_range_pattern(None)?,
            _ => {
                self.error_at_current("Expected match pattern.");
                return None;
            }
        };
        Some(start)
    }

    fn parse_range_pattern(&mut self, start: Option<Box<Expr<'gc>>>) -> Option<MatchPattern<'gc>> {
        let inclusive = self.match_token(TokenType::DotDotEq);
        if !inclusive && !self.match_token(TokenType::DotDot) {
            self.error_at_current("Expected '..' or '..=' in range pattern.");
            return None;
        }

        let end = if self.check(TokenType::FatArrow) || self.check(TokenType::Comma) {
            None
        } else {
            Some(Box::new(self.expression()?))
        };

        // Validate range values if both are literals
        if let (Some(box_start), Some(box_end)) = (&start, &end) {
            if let (
                Expr::Literal {
                    value: start_val, ..
                },
                Expr::Literal { value: end_val, .. },
            ) = (&**box_start, &**box_end)
            {
                match (start_val, end_val) {
                    (Literal::Number(s), Literal::Number(e)) if s > e => {
                        self.error("Invalid range pattern: start value must be less than or equal to end value.");
                    }
                    (Literal::Number(_), Literal::Number(_)) => {}
                    _ => {
                        self.error("Range patterns only support numeric values.");
                    }
                }
            }
        }

        Some(MatchPattern::Range {
            start,
            end,
            inclusive,
        })
    }

    fn parse_literal(&mut self, token: Token<'gc>) -> Option<Literal<'gc>> {
        match token.kind {
            TokenType::Number => {
                let value = token.lexeme.parse::<f64>().unwrap();
                Some(Literal::Number(value))
            }
            TokenType::String => Some(Literal::String(self.ctx.intern(token.lexeme.as_bytes()))),
            TokenType::True => Some(Literal::Boolean(true)),
            TokenType::False => Some(Literal::Boolean(false)),
            _ => None,
        }
    }

    fn error_type(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let name = self.previous;
        Some(Expr::Variable {
            name,
            line: self.previous.line,
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
        let is_in_static_method = if let Some(class_compiler) = self.class_compiler.as_ref() {
            if !class_compiler.is_enum && !class_compiler.has_superclass {
                self.error("Can't use 'super' in a class with no superclass.");
                return None;
            } else if class_compiler.is_enum {
                self.error("Can't use 'super' in an enum.");
                return None;
            }
            class_compiler.current_method_type.is_static_method() || self.fn_type.is_static_method()
        } else {
            self.error("Can't use 'super' outside of a class.");
            return None;
        };

        if is_in_static_method {
            self.error("Can't use 'super' in static method.");
            return None;
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

    fn self_(&mut self, _can_assign: bool) -> Option<Expr<'gc>> {
        let is_in_static_method = if let Some(class_compiler) = self.class_compiler.as_ref() {
            class_compiler.current_method_type.is_static_method() || self.fn_type.is_static_method()
        } else {
            self.error("Can't use 'self' outside of a class or enum.");
            return None;
        };

        if is_in_static_method {
            self.error("Can't use 'self' in static method.");
            return None;
        }

        Some(Expr::Self_ {
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
            if self.stop_at_brace && self.current.kind == TokenType::OpenBrace {
                // If we're in a flow condition and see a '{', stop here
                // to let the statement parser handle the block
                break;
            } else if self.in_match_arm && self.current.kind == TokenType::If {
                // If we're in a match arm and see a 'if', stop here to
                // avoid conflict with match arm's if guard
                break;
            }

            self.advance();
            // Do not reuse the previous rule since it may have changed.
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
}

// Precedence levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
enum Precedence {
    None,
    Assignment, // =
    Pipe,       // |>
    If,         // inline if/else
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
        TokenType::Dollar => ParseRule::new(Some(Parser::env_lookup), None, Precedence::Unary),
        TokenType::OpenBrace => ParseRule::new(
            Some(Parser::object),
            Some(Parser::object),
            Precedence::Primary,
        ),
        TokenType::OpenParen => {
            ParseRule::new(Some(Parser::grouping), Some(Parser::call), Precedence::Call)
        }
        TokenType::OpenBracket => {
            ParseRule::new(Some(Parser::bracket), Some(Parser::index), Precedence::Call)
        }
        TokenType::ColonColon => ParseRule::new(None, Some(Parser::enum_variant), Precedence::Call),
        TokenType::Pipe => ParseRule::new(Some(Parser::lambda), None, Precedence::None),
        TokenType::PipeArrow => ParseRule::new(None, Some(Parser::pipe_arrow), Precedence::Pipe),
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
        TokenType::Not => ParseRule::new(Some(Parser::unary), None, Precedence::None),
        TokenType::NotEqual => ParseRule::new(None, Some(Parser::binary), Precedence::Equality),
        TokenType::EqualEqual => ParseRule::new(None, Some(Parser::binary), Precedence::Equality),
        TokenType::Greater => ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        TokenType::GreaterEqual => {
            ParseRule::new(None, Some(Parser::binary), Precedence::Comparison)
        }
        TokenType::Less => ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        TokenType::LessEqual => ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        TokenType::Error => ParseRule::new(Some(Parser::error_type), None, Precedence::None),
        TokenType::Identifier => ParseRule::new(Some(Parser::variable), None, Precedence::None),
        TokenType::String => ParseRule::new(Some(Parser::string), None, Precedence::None),
        TokenType::FString => ParseRule::new(Some(Parser::fstring), None, Precedence::None),
        TokenType::RawString => ParseRule::new(Some(Parser::raw_string), None, Precedence::None),
        TokenType::Number => ParseRule::new(Some(Parser::number), None, Precedence::None),
        TokenType::And => ParseRule::new(None, Some(Parser::and), Precedence::And),
        TokenType::Or => ParseRule::new(None, Some(Parser::or), Precedence::Or),
        TokenType::Super => ParseRule::new(Some(Parser::super_), None, Precedence::None),
        TokenType::Self_ => ParseRule::new(Some(Parser::self_), None, Precedence::None),
        TokenType::True | TokenType::False | TokenType::Nil => {
            ParseRule::new(Some(Parser::literal), None, Precedence::None)
        }
        TokenType::If => ParseRule::new(None, Some(Parser::inline_if), Precedence::If),
        TokenType::Match => ParseRule::new(
            Some(Parser::match_expression),
            None,
            // Same precedence as inline if expressions
            Precedence::If,
        ),
        TokenType::In => ParseRule::new(None, Some(Parser::binary), Precedence::Comparison),
        TokenType::Prompt => ParseRule::new(Some(Parser::prompt), None, Precedence::None),
        _ => ParseRule::new(None, None, Precedence::None),
    }
}
