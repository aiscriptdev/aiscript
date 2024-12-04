use std::{
    collections::{HashMap, HashSet},
    mem,
    sync::atomic::{AtomicU16, Ordering},
};

use crate::{
    ai::Agent,
    ast::{
        AgentDecl, ChunkId, ClassDecl, EnumDecl, ErrorHandler, FunctionDecl, Mutability,
        ObjectProperty, VariableDecl,
    },
    object::{Enum, EnumVariant, Function, FunctionType, Upvalue},
    vm::{Context, VmError},
    OpCode, Value,
};
use crate::{
    ast::{Expr, FnDef, Literal, Parameter, Program, Stmt},
    lexer::{Token, TokenType},
    ty::{Type, TypeResolver},
};
use gc_arena::{Gc, RefLock};
use indexmap::IndexMap;

const MAX_LOCALS: usize = u8::MAX as usize + 1;
const UNINITIALIZED_LOCAL_DEPTH: isize = -1;
pub static CHUNK_ID: AtomicU16 = AtomicU16::new(0);

#[derive(Debug, Clone, Default)]
struct Local<'gc> {
    name: Token<'gc>,
    depth: isize,
    is_captured: bool,
    mutability: Mutability,
}

#[derive(Debug, Default)]
struct LoopScope {
    // Position of increment code for continue
    increment: usize,
    // Break jump positions to patch
    breaks: Vec<usize>,
}

pub struct CodeGen<'gc> {
    ctx: Context<'gc>,
    chunks: HashMap<ChunkId, Function<'gc>>,
    // <function mangled name, chunk_id>
    named_id_map: HashMap<String, FnDef>,
    type_resolver: TypeResolver<'gc>,
    function: Function<'gc>,
    fn_type: FunctionType,
    locals: [Local<'gc>; MAX_LOCALS],
    local_count: usize,
    scope_depth: isize,
    loop_scopes: Vec<LoopScope>,
    // Track constant globals
    const_globals: HashSet<&'gc str>,
    enclosing: Option<Box<CodeGen<'gc>>>,
    current_line: u32,
    had_error: bool,
}

impl<'gc> CodeGen<'gc> {
    pub fn new(ctx: Context<'gc>, fn_type: FunctionType, name: &str) -> Box<Self> {
        let generator = Box::new(CodeGen {
            ctx,
            chunks: HashMap::new(),
            named_id_map: HashMap::new(),
            type_resolver: TypeResolver::new(),
            function: Function::new(ctx.intern(name.as_bytes()), 0),
            fn_type,
            locals: std::array::from_fn(|i| {
                // The compiler’s locals array keeps track of which stack slots
                // are associated with which local variables or temporaries.
                // From now on, the compiler implicitly claims stack slot zero for the VM’s own
                // internal use. We give it an empty name so that the user can’t write an
                // identifier that refers to it.
                if i == 0 {
                    let name = if fn_type.is_constructor() || fn_type.is_method() {
                        // Slot zero will store the instance in class methods.
                        Token::new(TokenType::Self_, "self", 0)
                    } else {
                        Token::default()
                    };
                    Local {
                        name,
                        ..Local::default()
                    }
                } else {
                    Local::default()
                }
            }),
            // The initial value of the local_count starts at 1
            // because we reserve slot zero for VM use.
            local_count: 1,
            scope_depth: 0,
            loop_scopes: Vec::new(),
            const_globals: HashSet::new(),
            enclosing: None,
            current_line: 0,
            had_error: false,
        });

        generator
    }

    pub fn generate(
        program: Program<'gc>,
        ctx: Context<'gc>,
    ) -> Result<HashMap<ChunkId, Function<'gc>>, VmError> {
        // Reset CHUNK_ID initial value to get the same id for repeat compile
        CHUNK_ID.store(0, Ordering::Relaxed);
        let mut generator = Self::new(ctx, FunctionType::Script, "script");

        for stmt in &program.statements {
            generator.declare_class_and_enum(stmt)?;
        }

        for stmt in &program.statements {
            generator.declare_functions(stmt)?;
        }

        for stmt in program.statements {
            generator.generate_stmt(stmt)?;
        }

        generator.emit_return();

        if generator.had_error {
            Err(VmError::CompileError)
        } else {
            let function = mem::take(&mut generator.function);
            generator
                .chunks
                .insert(CHUNK_ID.fetch_add(1, Ordering::AcqRel), function);
            Ok(generator.chunks)
        }
    }

    fn declare_class_and_enum(&mut self, stmt: &Stmt<'gc>) -> Result<(), VmError> {
        match stmt {
            Stmt::Class(ClassDecl { name, .. }) | Stmt::Enum(EnumDecl { name, .. }) => {
                self.type_resolver
                    .register_type(name.lexeme, Type::Custom(*name));
            }
            _ => (),
        }
        Ok(())
    }

    fn declare_functions(&mut self, stmt: &Stmt<'gc>) -> Result<(), VmError> {
        match stmt {
            Stmt::Block { statements, .. } => {
                for stmt in statements {
                    self.declare_functions(stmt)?;
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.declare_functions(then_branch)?;
                if let Some(else_branch) = else_branch {
                    self.declare_functions(else_branch)?;
                }
            }
            Stmt::Loop { body, .. } => {
                self.declare_functions(body)?;
            }
            Stmt::Function(FunctionDecl {
                name,
                mangled_name,
                body,
                doc,
                params,
                ..
            }) => {
                if !self.named_id_map.contains_key(mangled_name) {
                    let chunk_id = CHUNK_ID.fetch_add(1, Ordering::AcqRel);
                    self.named_id_map
                        .insert(mangled_name.to_owned(), FnDef::new(chunk_id, doc, params));
                } else {
                    self.error_at(*name, "A function with same name already exists.");
                }

                for stmt in body {
                    self.declare_functions(stmt)?;
                }
            }
            Stmt::Enum(EnumDecl { methods, .. }) => {
                for methods in methods {
                    self.declare_functions(methods)?;
                }
            }
            Stmt::Class(ClassDecl { methods, .. }) => {
                for methods in methods {
                    self.declare_functions(methods)?;
                }
            }
            Stmt::Agent(AgentDecl { tools, .. }) => {
                for tool in tools {
                    self.declare_functions(tool)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn generate_stmt<S: Into<Stmt<'gc>>>(&mut self, stmt: S) -> Result<(), VmError> {
        let stmt = stmt.into();
        self.current_line = stmt.line();
        match stmt {
            Stmt::Use { path, .. } => {
                // Load the module name as a constant
                let module_name = self.identifier_constant(path.lexeme);
                self.emit(OpCode::ImportModule(module_name as u8));
            }
            Stmt::Break { .. } => {
                let exit_jump = self.emit_jump(OpCode::Jump(0));
                // Get the last scope's index
                let last_idx = self.loop_scopes.len() - 1;
                // Add the break jump to the current loop scope
                self.loop_scopes[last_idx].breaks.push(exit_jump);
            }
            Stmt::Continue { .. } => {
                while self.local_count > 0
                    && self.locals[self.local_count - 1].depth > self.scope_depth
                {
                    self.emit(OpCode::Pop(1));
                    self.local_count -= 1;
                }

                if let Some(loop_scope) = self.loop_scopes.last() {
                    self.emit_loop(loop_scope.increment);
                }
            }
            Stmt::Expression { expression, .. } => {
                self.generate_expr(expression)?;
                // an expression statement evaluates the expression and discards the result
                // since the result already exists in the stack, we can just pop it
                self.emit(OpCode::Pop(1));
            }
            Stmt::Let(VariableDecl {
                name,
                initializer,
                visibility,
                ..
            }) => {
                self.declare_variable(name, Mutability::Mutable);
                if let Some(initial_value) = initializer {
                    self.generate_expr(initial_value)?;
                } else {
                    self.emit(OpCode::Nil);
                }
                if self.scope_depth > 0 {
                    self.mark_initialized();
                } else {
                    let global = self.identifier_constant(name.lexeme);
                    self.emit(OpCode::DefineGlobal {
                        name_constant: global as u8,
                        visibility,
                    });
                }
            }
            Stmt::Const {
                name,
                initializer,
                visibility,
                ..
            } => {
                self.declare_variable(name, Mutability::Immutable);
                self.generate_expr(initializer)?;
                if self.scope_depth > 0 {
                    self.mark_initialized();
                } else {
                    self.const_globals.insert(name.lexeme);
                    let global = self.identifier_constant(name.lexeme);
                    self.emit(OpCode::DefineGlobal {
                        name_constant: global as u8,
                        visibility,
                    });
                }
            }
            Stmt::Block { statements, .. } => {
                self.begin_scope();
                for stmt in statements {
                    self.generate_stmt(stmt)?;
                }
                self.end_scope();
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.generate_expr(condition)?;
                let then_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                self.emit(OpCode::Pop(1));
                self.generate_stmt(then_branch)?;

                let else_jump = self.emit_jump(OpCode::Jump(0));
                self.patch_jump(then_jump);
                self.emit(OpCode::Pop(1));

                if let Some(else_branch) = else_branch {
                    self.generate_stmt(else_branch)?;
                }
                self.patch_jump(else_jump);
            }
            Stmt::Loop {
                initializer,
                condition,
                increment,
                body,
                ..
            } => {
                self.begin_scope();

                // Initialize if needed
                if let Some(initial_value) = initializer {
                    self.generate_stmt(initial_value)?;
                }

                let loop_start = self.function.code_size();
                let mut loop_scope = LoopScope {
                    increment: loop_start, // Will be updated for increment
                    breaks: Vec::new(),
                };

                // Generate condition
                self.generate_expr(condition)?;
                let exit_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                self.emit(OpCode::Pop(1));

                if let Some(incr) = increment {
                    let body_jump = self.emit_jump(OpCode::Jump(0));
                    let increment_start = self.function.code_size();

                    // Generate increment code
                    self.generate_expr(incr)?;
                    self.emit(OpCode::Pop(1));

                    self.emit_loop(loop_start);

                    // Update increment position to point to increment code
                    loop_scope.increment = increment_start;

                    self.patch_jump(body_jump);
                }

                self.loop_scopes.push(loop_scope);

                // Generate body
                self.generate_stmt(body)?;

                // Jump back to increment or condition
                self.emit_loop(self.loop_scopes.last().unwrap().increment);

                // Patch breaks and cleanup
                self.patch_jump(exit_jump);
                self.emit(OpCode::Pop(1));

                // Patch all break statements
                if let Some(scope) = self.loop_scopes.pop() {
                    for break_jump in scope.breaks {
                        self.patch_jump(break_jump);
                    }
                }

                self.end_scope();
            }
            Stmt::Function(FunctionDecl {
                name,
                mangled_name,
                params,
                return_type,
                body,
                fn_type,
                visibility,
                ..
            }) => {
                self.declare_variable(name, Mutability::default());
                if self.scope_depth > 0 {
                    self.mark_initialized();
                }

                self.generate_function(
                    name.lexeme,
                    &mangled_name,
                    &params,
                    return_type,
                    body,
                    fn_type,
                )?;

                if self.scope_depth == 0 {
                    let global = self.identifier_constant(name.lexeme);
                    self.emit(OpCode::DefineGlobal {
                        name_constant: global as u8,
                        visibility,
                    });
                }
            }
            Stmt::Raise { error, .. } => {
                self.generate_expr(error)?;
                self.emit(OpCode::Return);
            }
            Stmt::Return { value, .. } => {
                if let Some(expr) = value {
                    self.generate_expr(expr)?;
                    self.emit(OpCode::Return);
                } else {
                    self.emit_return();
                }
            }
            Stmt::Enum(EnumDecl {
                name,
                variants,
                methods,
                visibility,
                ..
            }) => {
                // Emit enum declaration
                let enum_ = Gc::new(
                    &self.ctx,
                    RefLock::new(Enum {
                        name: self.ctx.intern(name.lexeme.as_bytes()),
                        variants: variants
                            .iter()
                            .map(|v| {
                                (
                                    self.ctx.intern(v.name.lexeme.as_bytes()),
                                    Value::from(&v.value),
                                )
                            })
                            .collect(),
                        methods: HashMap::default(),
                        static_methods: HashMap::default(),
                    }),
                );
                self.type_resolver.register_enum(name.lexeme, enum_);
                let enum_constant = self.make_constant(Value::Enum(enum_));
                self.emit(OpCode::Enum(enum_constant as u8));

                let name_constant = self.identifier_constant(name.lexeme);

                // Define globally right away
                self.emit(OpCode::DefineGlobal {
                    name_constant: name_constant as u8,
                    visibility,
                });

                // Load enum again for method definitions
                self.emit(OpCode::GetGlobal(name_constant as u8));

                for method in methods {
                    if let Stmt::Function(function_decl) = method {
                        self.generate_method(function_decl)?;
                    }
                }
                // Pop the enum
                self.emit(OpCode::Pop(1));
            }
            Stmt::Class(class_decl) => {
                self.generate_class(class_decl)?;
            }
            Stmt::Agent(AgentDecl {
                name,
                mangled_name,
                fields,
                tools,
                visibility,
                ..
            }) => {
                // Emit agent declaration
                let agent_name = self.ctx.intern(name.lexeme.as_bytes());
                let mut agent = Agent::new(&self.ctx, agent_name)
                    .parse_instructions(&fields)
                    .parse_model(&fields)
                    .parse_tools(&fields, |name| {
                        let mut scopes = mangled_name.split("$").collect::<Vec<_>>();
                        loop {
                            if scopes.is_empty() {
                                self.error_at(
                                    *name,
                                    &format!("Unable to find the function called {}", name.lexeme),
                                );
                                return None;
                            }
                            scopes.pop();
                            let n = format!("{}${}", scopes.join("$"), name.lexeme);
                            if let Some(fn_def) = self.named_id_map.get(&n) {
                                return Some(fn_def.clone());
                            }
                        }
                    });

                let tool_count = tools.len();
                for tool in tools {
                    if let Stmt::Function(FunctionDecl {
                        name,
                        mangled_name,
                        doc,
                        params,
                        return_type,
                        body,
                        ..
                    }) = tool
                    {
                        let fn_type = FunctionType::Tool;
                        let chunk_id = self.generate_function(
                            name.lexeme,
                            &mangled_name,
                            &params,
                            return_type,
                            body,
                            fn_type,
                        )?;
                        if agent
                            .tools
                            .insert(name.lexeme.to_string(), FnDef::new(chunk_id, &doc, &params))
                            .is_some()
                        {
                            self.error_at(name, &format!("Duplicate tool name: {}", name.lexeme));
                        }
                    }
                }
                // Pop tool functions from stack because we never
                // define global for this tool function.
                self.emit(OpCode::Pop(tool_count as u8));
                let agent = Gc::new(&self.ctx, agent);
                let agent_constant = self.make_constant(Value::from(agent));
                self.emit(OpCode::Agent(agent_constant as u8));
                let name_constant = self.identifier_constant(name.lexeme);
                self.emit(OpCode::DefineGlobal {
                    name_constant: name_constant as u8,
                    visibility,
                });
                // self.emit(OpCode::Pop);
            }
        }
        Ok(())
    }

    fn generate_expr<E: Into<Expr<'gc>>>(&mut self, expr: E) -> Result<(), VmError> {
        let expr = expr.into();
        self.current_line = expr.line();
        match expr {
            Expr::Array { elements, .. } => {
                let len = elements.len();
                // Generate code for each element
                for element in elements {
                    self.generate_expr(element)?;
                }
                self.emit(OpCode::MakeArray(len as u8));
            }
            Expr::EnumVariant {
                enum_name, variant, ..
            } => {
                self.validate_enum_variant(enum_name, variant);
                self.named_variable(enum_name, false)?;

                let name_constant = self.identifier_constant(variant.lexeme) as u8;
                self.emit(OpCode::EnumVariant {
                    name_constant,
                    evaluate: false,
                });
            }
            Expr::Object { properties, .. } => {
                let len = properties.len();
                // For each property, first emit key and value onto stack
                for property in properties {
                    match property {
                        ObjectProperty::Literal { key, value } => {
                            // For literal key, emit as constant string
                            let key_constant = self.identifier_constant(key.lexeme);
                            self.emit(OpCode::Constant(key_constant as u8));

                            // Generate value code
                            self.generate_expr(value)?;
                        }
                        ObjectProperty::Computed { key_expr, value } => {
                            // Generate key expression code
                            self.generate_expr(key_expr)?;

                            // Generate value code
                            self.generate_expr(value)?;
                        }
                    }
                }

                // Now create object with all properties
                // Stack has pairs of [key1, value1, key2, value2, ...]
                self.emit(OpCode::MakeObject(len as u8));
            }
            Expr::Binary {
                left,
                operator,
                right,
                ..
            } => {
                self.generate_expr(left)?;
                self.generate_expr(right)?;
                match operator.kind {
                    TokenType::Plus => self.emit(OpCode::Add),
                    TokenType::Minus => self.emit(OpCode::Subtract),
                    TokenType::Star => self.emit(OpCode::Multiply),
                    TokenType::StarStar => self.emit(OpCode::Power),
                    TokenType::Slash => self.emit(OpCode::Divide),
                    TokenType::Percent => self.emit(OpCode::Modulo),
                    TokenType::NotEqual => self.emit(OpCode::NotEqual),
                    TokenType::EqualEqual => self.emit(OpCode::Equal),
                    TokenType::Greater => self.emit(OpCode::Greater),
                    TokenType::GreaterEqual => self.emit(OpCode::GreaterEqual),
                    TokenType::Less => self.emit(OpCode::Less),
                    TokenType::LessEqual => self.emit(OpCode::LessEqual),
                    TokenType::In => self.emit(OpCode::In),
                    _ => {
                        self.error_at(
                            operator,
                            &format!("Invalid binary operator: {}", operator.lexeme),
                        );
                    }
                }
            }
            Expr::Grouping { expression, .. } => {
                self.generate_expr(expression)?;
            }
            Expr::Literal { value, .. } => match value {
                Literal::Number(n) => self.emit_constant(Value::from(n)),
                Literal::String(s) => self.emit_constant(Value::from(s)),
                Literal::Boolean(b) => self.emit(OpCode::Bool(b)),
                Literal::Nil => self.emit(OpCode::Nil),
            },
            Expr::Unary {
                operator, right, ..
            } => {
                self.generate_expr(right)?;
                match operator.kind {
                    TokenType::Minus => self.emit(OpCode::Negate),
                    TokenType::Not => self.emit(OpCode::Not),
                    _ => {
                        self.error_at(
                            operator,
                            &format!("Invalid unary operator: {}", operator.lexeme),
                        );
                    }
                }
            }
            Expr::Variable { name, .. } => {
                self.named_variable(name, false)?;
            }
            Expr::Assign { name, value, .. } => {
                self.generate_expr(value)?;
                self.named_variable(name, true)?;
            }
            Expr::Block { statements, .. } => {
                let last_is_return = matches!(statements.last(), Some(Stmt::Return { .. }));
                // Generate code for each statement in the block
                for stmt in statements {
                    self.generate_stmt(stmt)?;
                }

                // If the block doesn't end with a return statement,
                // implicitly return nil
                if !last_is_return {
                    self.emit(OpCode::Nil);
                }
            }
            Expr::Lambda { params, body, .. } => {
                // Create a new compiler for the lambda
                let name = format!("lambda_{}", CHUNK_ID.load(Ordering::Relaxed));
                let chunk_id = CHUNK_ID.fetch_add(1, Ordering::AcqRel);

                // Create the lambda compiler and swap with self
                let mut lambda_compiler = Self::new(self.ctx, FunctionType::Lambda, &name);
                lambda_compiler.named_id_map = self.named_id_map.clone();

                // Store current compiler as enclosing and set enclosing for lambda
                let current_compiler = mem::replace(self, *lambda_compiler);
                self.enclosing = Some(Box::new(current_compiler));

                // Set up function parameters
                self.function.arity = params.len() as u8;
                self.function.max_arity = params.len() as u8;

                // Add parameters as locals
                self.begin_scope();
                for param in params {
                    self.declare_variable(param, Mutability::Mutable);
                    self.mark_initialized();
                }

                // Generate code for the body (which is a Block expression)
                self.generate_expr(body)?;

                // Always end with return
                self.emit(OpCode::Return);

                self.end_scope();

                // Check for errors
                if self.had_error {
                    return Err(VmError::CompileError);
                }

                // Get the generated function and chunks
                self.function.shrink_to_fit();
                let generated_function = mem::take(&mut self.function);
                let generated_chunks = mem::take(&mut self.chunks);

                // Get the enclosing compiler back
                if let Some(enclosing) = self.enclosing.take() {
                    let _ = mem::replace(self, *enclosing);
                }

                // Store the generated function and extend chunks
                self.chunks.insert(chunk_id, generated_function);
                self.chunks.extend(generated_chunks);

                // Emit closure instruction
                self.emit(OpCode::Closure { chunk_id });
            }
            Expr::Call {
                callee,
                arguments,
                keyword_args,
                error_handler,
                ..
            } => {
                self.generate_call(callee, arguments, keyword_args, error_handler)?;
            }
            Expr::Invoke {
                object,
                method,
                arguments,
                keyword_args,
                error_handler,
                ..
            } => {
                self.generate_invoke(object, method, arguments, keyword_args, error_handler)?;
            }
            Expr::Index {
                object, key, value, ..
            } => {
                self.generate_expr(object)?; // Push object
                self.generate_expr(key)?; // Push key

                if let Some(val) = value {
                    self.generate_expr(val)?; // Push value if it's a set operation
                    self.emit(OpCode::SetIndex);
                } else {
                    self.emit(OpCode::GetIndex);
                }
            }
            Expr::InlineIf {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.generate_expr(condition)?;
                let then_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                self.emit(OpCode::Pop(1));

                self.generate_expr(then_branch)?;
                let else_jump = self.emit_jump(OpCode::Jump(0));

                self.patch_jump(then_jump);
                self.emit(OpCode::Pop(1));

                self.generate_expr(else_branch)?;
                self.patch_jump(else_jump);
            }
            Expr::EvaluateVariant { expr, .. } => {
                if let Expr::EnumVariant {
                    enum_name, variant, ..
                } = *expr
                {
                    // Evaluate enum variant directly:
                    // enum E { A = 1 }
                    // print([E::A]) get 1
                    self.validate_enum_variant(enum_name, variant);

                    self.named_variable(enum_name, false)?;
                    let name_constant = self.identifier_constant(variant.lexeme) as u8;
                    self.emit(OpCode::EnumVariant {
                        name_constant,
                        evaluate: true,
                    });
                } else {
                    // Evaluate variant variable:
                    // enum E { A = 1 }
                    // let x = E::A;
                    // print([x]); get 1
                    self.generate_expr(expr)?;
                    self.emit(OpCode::EnumVariant {
                        name_constant: 0,
                        evaluate: true,
                    });
                }
            }
            Expr::Get { object, name, .. } => {
                self.generate_expr(object)?;
                let name_constant = self.identifier_constant(name.lexeme);
                self.emit(OpCode::GetProperty(name_constant as u8));
            }
            Expr::Set {
                object,
                name,
                value,
                ..
            } => {
                self.generate_expr(object)?;
                self.generate_expr(value)?;
                let name_constant = self.identifier_constant(name.lexeme);
                self.emit(OpCode::SetProperty(name_constant as u8));
            }
            Expr::Self_ { .. } => {
                // we can’t assign to 'self', so we pass can_assign=false to disallow
                // look for a following = operator in the expression
                self.named_variable(Token::new(TokenType::Self_, "self", 0), false)?;
            }
            Expr::Super { method, .. } => {
                // Get the receiver ('self')
                self.emit(OpCode::GetLocal(0));

                // Look up 'super' in upvalues
                let method_constant = self.identifier_constant(method.lexeme);
                if let Some((pos, _, _)) = self
                    .resolve_upvalue("super")
                    .inspect_err(|err| self.error(err))
                    .ok()
                    .flatten()
                {
                    self.emit(OpCode::GetUpvalue(pos));
                } else {
                    self.error("Unable to resolve 'super'");
                    return Err(VmError::CompileError);
                }

                self.emit(OpCode::GetSuper(method_constant as u8));
            }
            Expr::SuperInvoke {
                method,
                arguments,
                keyword_args,
                ..
            } => {
                // Get this instance
                self.emit(OpCode::GetLocal(0));

                let positional_count = arguments.len() as u8;
                let keyword_count = keyword_args.len() as u8;
                // Generate arguments
                for arg in arguments {
                    self.generate_expr(arg)?;
                }
                self.generate_keyword_args(keyword_args)?;

                // Get superclass and invoke method
                if let Some((pos, _, _)) = self
                    .resolve_upvalue("super")
                    .map_err(|e| VmError::RuntimeError(e.into()))?
                {
                    self.emit(OpCode::GetUpvalue(pos));
                } else {
                    return Err(VmError::RuntimeError("Unable to resolve 'super'".into()));
                }

                let method_constant = self.identifier_constant(method.lexeme);
                self.emit(OpCode::SuperInvoke {
                    method_constant: method_constant as u8,
                    positional_count,
                    keyword_count,
                });
            }
            Expr::And { left, right, .. } => {
                self.generate_expr(left)?;
                let end_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                self.emit(OpCode::Pop(1));
                self.generate_expr(right)?;
                self.patch_jump(end_jump);
            }
            Expr::Or { left, right, .. } => {
                self.generate_expr(left)?;
                let else_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                let end_jump = self.emit_jump(OpCode::Jump(0));

                self.patch_jump(else_jump);
                self.emit(OpCode::Pop(1));
                self.generate_expr(right)?;
                self.patch_jump(end_jump);
            }
            Expr::Prompt { expression, .. } => {
                self.generate_expr(expression)?;
                self.emit(OpCode::Prompt);
            }
        }
        Ok(())
    }

    fn generate_keyword_args(
        &mut self,
        keyword_args: HashMap<String, Expr<'gc>>,
    ) -> Result<(), VmError> {
        for (name, value) in keyword_args {
            let name_constant = self.identifier_constant(&name);
            self.emit(OpCode::Constant(name_constant as u8));
            self.generate_expr(value)?;
        }
        Ok(())
    }
}

impl<'gc> CodeGen<'gc> {
    fn generate_call(
        &mut self,
        callee: Box<Expr<'gc>>,
        arguments: Vec<Expr<'gc>>,
        keyword_args: HashMap<String, Expr<'gc>>,
        error_handler: Option<ErrorHandler<'gc>>,
    ) -> Result<(), VmError> {
        let arg_count = arguments.len() as u8;
        let kw_count = keyword_args.len() as u8;
        self.generate_expr(callee)?;
        for arg in arguments {
            self.generate_expr(arg)?;
        }
        self.generate_keyword_args(keyword_args)?;

        // Emit call instruction - result will be on stack
        self.emit(OpCode::Call {
            positional_count: arg_count,
            keyword_count: kw_count,
        });

        if let Some(handler) = error_handler {
            self.generate_error_handler(handler)?;
        }
        Ok(())
    }

    fn generate_invoke(
        &mut self,
        object: Box<Expr<'gc>>,
        method: Token<'gc>,
        arguments: Vec<Expr<'gc>>,
        keyword_args: HashMap<String, Expr<'gc>>,
        error_handler: Option<ErrorHandler<'gc>>,
    ) -> Result<(), VmError> {
        let arg_count = arguments.len() as u8;
        let kw_count = keyword_args.len() as u8;

        self.generate_expr(object)?;
        for arg in arguments {
            self.generate_expr(arg)?;
        }
        self.generate_keyword_args(keyword_args)?;

        let method_const = self.identifier_constant(method.lexeme);

        self.emit(OpCode::Invoke {
            method_constant: method_const as u8,
            positional_count: arg_count,
            keyword_count: kw_count,
        });

        if let Some(handler) = error_handler {
            self.generate_error_handler(handler)?;
        }
        Ok(())
    }

    fn generate_error_handler(&mut self, handler: ErrorHandler<'gc>) -> Result<(), VmError> {
        let error_jump = self.emit_jump(OpCode::JumpIfError(0));
        let end_jump = self.emit_jump(OpCode::Jump(0));

        self.patch_jump(error_jump);
        if handler.propagate {
            // For ? operator, return error directly
            self.emit(OpCode::Return);
        } else {
            // Begin scope for error variable
            self.begin_scope();
            // Store error in handler variable
            self.declare_variable(handler.error_var, Mutability::Mutable);
            self.mark_initialized();

            let has_return = matches!(handler.handler_body.last(), Some(Stmt::Return { .. }));
            // Generate handler body - any return here will return from entire function
            for stmt in handler.handler_body {
                self.generate_stmt(stmt)?;
            }

            // If no return in handler, set nil as value and continue
            if !has_return {
                self.emit(OpCode::Nil);
            }
            self.end_scope();
        }
        self.patch_jump(end_jump);
        Ok(())
    }
    fn generate_class(
        &mut self,
        ClassDecl {
            name,
            superclass,
            methods,
            visibility,
            ..
        }: ClassDecl<'gc>,
    ) -> Result<(), VmError> {
        // Emit class declaration
        let name_constant = self.identifier_constant(name.lexeme);
        self.emit(OpCode::Class(name_constant as u8));
        self.emit(OpCode::DefineGlobal {
            name_constant: name_constant as u8,
            visibility,
        });

        let has_superclass = superclass.is_some();
        // Handle inheritance
        if let Some(superclass) = superclass {
            // Begin a new scope for 'super'
            self.begin_scope();

            // First get the superclass
            self.generate_expr(superclass)?;

            // Creating a new lexical scope ensures that if we declare two classes in the same scope,
            // each has a different local slot to store its superclass. Since we always name this
            // variable “super”, if we didn’t make a scope for each subclass, the variables would collide.
            let super_token = Token::new(TokenType::Super, "super", name.line);
            self.declare_variable(super_token, Mutability::Immutable);
            self.mark_initialized();

            // Then get the class we just defined
            self.emit(OpCode::GetGlobal(name_constant as u8));

            // Emit inherit instruction
            self.emit(OpCode::Inherit);

            // Load class again for method definitions
            self.emit(OpCode::GetGlobal(name_constant as u8));
        } else {
            // Load class for method definitions
            self.emit(OpCode::GetGlobal(name_constant as u8));
        }

        // Generate methods
        for method in methods {
            if let Stmt::Function(function_decl) = method {
                self.generate_method(function_decl)?;
            }
        }

        // Once we’ve reached the end of the methods, we no longer need
        // the class and tell the VM to pop it off the stack.
        self.emit(OpCode::Pop(1));

        // Close the scope created for 'super' if there was inheritance
        if has_superclass {
            self.end_scope();
        }
        Ok(())
    }

    fn generate_method(
        &mut self,
        FunctionDecl {
            name,
            mangled_name,
            params,
            return_type,
            body,
            fn_type,
            ..
        }: FunctionDecl<'gc>,
    ) -> Result<(), VmError> {
        self.generate_function(
            name.lexeme,
            &mangled_name,
            &params,
            return_type,
            body,
            fn_type,
        )?;
        let method_constant = self.identifier_constant(name.lexeme);
        self.emit(OpCode::Method {
            name_constant: method_constant as u8,
            is_static: fn_type.is_static_method(),
        });
        Ok(())
    }

    fn generate_function(
        &mut self,
        name: &'gc str,
        mangle_name: &str,
        params: &IndexMap<Token<'gc>, Parameter<'gc>>,
        return_type: Option<Token<'gc>>,
        body: Vec<Stmt<'gc>>,
        fn_type: FunctionType,
    ) -> Result<ChunkId, VmError> {
        // Validate parameter types
        for (param_token, param) in params {
            if let Some(param_type) = param.type_hint.as_ref().copied() {
                let ty = Type::from_token(param_type);
                if let Err(err) = self.type_resolver.validate_type(ty) {
                    self.error_at(
                        *param_token,
                        &format!("Invalid parameter type '{}': {}.", ty.type_name(), err),
                    );
                }
            }
        }

        // Validate return type if present
        if let Some(ret_type) = return_type.as_ref().copied() {
            let ty = Type::from_token(ret_type);
            if let Err(err) = self.type_resolver.validate_type(ty) {
                self.error_at(
                    ret_type,
                    &format!("Invalid return type '{}': {}.", ty.type_name(), err),
                );
            }
        }
        let compiler = Self::new(self.ctx, fn_type, name);

        // Create a new compiler taking ownership of current one
        let mut enclosing = mem::replace(self, *compiler);
        self.named_id_map = mem::take(&mut enclosing.named_id_map);
        self.type_resolver = mem::take(&mut enclosing.type_resolver);
        self.enclosing = Some(Box::new(enclosing));

        self.begin_scope();

        // Store parameter count and default value count
        let param_count = params.len();
        let default_count = params
            .values()
            .filter(|p| p.default_value.is_some())
            .count();
        self.function.arity = (param_count - default_count) as u8;
        self.function.max_arity = param_count as u8;

        // Compile parameters and their default values
        for (index, param) in params.values().enumerate() {
            self.declare_variable(param.name, Mutability::Mutable);
            self.mark_initialized();

            let name = self.ctx.intern(param.name.lexeme.as_bytes());
            // Store default value if present
            if let Some(expr) = &param.default_value {
                let default_value = match expr {
                    Expr::Literal { value, .. } => Value::from(value),
                    Expr::EnumVariant {
                        enum_name, variant, ..
                    } => {
                        if let Some(enum_) = self.type_resolver.get_enum(enum_name.lexeme) {
                            let variant_name = self.ctx.intern(variant.lexeme.as_bytes());
                            let variant_value = match enum_.borrow().get_variant_value(variant_name)
                            {
                                Some(value) => value,
                                None => {
                                    self.error_at(
                                        *variant,
                                        &format!(
                                            "No variant called '{}' in enum '{}'.",
                                            variant.lexeme, enum_name.lexeme
                                        ),
                                    );
                                    Value::default()
                                }
                            };
                            Value::EnumVariant(Gc::new(
                                &self.ctx,
                                EnumVariant {
                                    enum_,
                                    name: variant_name,
                                    value: variant_value,
                                },
                            ))
                        } else {
                            self.error_at(
                                *enum_name,
                                &format!("Invalid enum '{}'.", enum_name.lexeme),
                            );
                            Value::default()
                        }
                    }
                    _ => unreachable!(),
                };
                self.function
                    .params
                    .insert(name, (index as u8, default_value));
            } else {
                self.function.params.insert(name, (index as u8, Value::Nil));
            }
        }

        // Compile function body
        for stmt in body {
            self.generate_stmt(stmt)?;
        }

        self.emit_return();

        // Restore the original compiler
        if self.had_error {
            return Err(VmError::CompileError);
        }
        let mut chunk_id = 0;
        if let Some(mut enclosing) = self.enclosing.take() {
            self.function.shrink_to_fit();
            let function = mem::take(&mut self.function);
            chunk_id = self
                .named_id_map
                .get(mangle_name)
                .map(|n| n.chunk_id)
                .unwrap();
            // TODO: Duplicate function name?
            self.chunks.insert(chunk_id, function);
            enclosing.named_id_map = mem::take(&mut self.named_id_map);
            enclosing.type_resolver = mem::take(&mut self.type_resolver);
            let chunks = mem::take(&mut self.chunks);
            *self = *enclosing;
            self.chunks.extend(chunks);
            self.emit(OpCode::Closure { chunk_id });
        }
        Ok(chunk_id)
    }

    fn validate_enum_variant(&mut self, enum_name: Token<'gc>, variant: Token<'gc>) {
        // Validate enums and variants
        if let Some(enum_) = self.type_resolver.get_enum(enum_name.lexeme) {
            let variant_name = self.ctx.intern(variant.lexeme.as_bytes());
            if enum_.borrow().get_variant_value(variant_name).is_none() {
                self.error_at(
                    variant,
                    &format!(
                        "No variant called '{}' in enum '{}'.",
                        variant.lexeme, enum_name.lexeme
                    ),
                );
            }
        } else {
            self.error_at(enum_name, &format!("Invalid enum '{}'.", enum_name.lexeme));
        }
    }

    // Bytecode emission methods
    fn emit(&mut self, op: OpCode) {
        self.function.write_byte(op, self.current_line);
    }

    fn emit_constant(&mut self, value: Value<'gc>) {
        let constant = self.make_constant(value);
        self.emit(OpCode::Constant(constant as u8));
    }

    fn emit_return(&mut self) {
        if self.fn_type == FunctionType::Constructor {
            self.emit(OpCode::GetLocal(0));
        } else {
            self.emit(OpCode::Nil);
        }
        self.emit(OpCode::Return);
    }

    fn emit_jump(&mut self, instruction: OpCode) -> usize {
        self.emit(instruction);
        self.function.code_size()
    }

    fn patch_jump(&mut self, offset: usize) {
        let jump = self.function.code_size() - offset;
        if jump > u16::MAX as usize {
            self.error("Too much code to jump over.");
        }
        self.function[offset - 1].putch_jump(jump as u16);
    }

    fn emit_loop(&mut self, loop_start: usize) {
        let offset = self.function.code_size() - loop_start + 1;
        if offset > u16::MAX as usize {
            self.error("Loop body too large.");
        }
        self.emit(OpCode::Loop(offset as u16));
    }

    // Variable handling methods
    fn named_variable(&mut self, name: Token<'gc>, can_assign: bool) -> Result<(), VmError> {
        let (get_op, set_op) =
            if let Some((pos, depth, mutability)) = self.resolve_local(name.lexeme) {
                if depth == UNINITIALIZED_LOCAL_DEPTH {
                    self.error_at(name, "Can't read local variable in its own initializer.");
                }

                if can_assign && mutability == Mutability::Immutable {
                    self.error_at(name, "Cannot assign to constant variable.");
                }
                (OpCode::GetLocal(pos), OpCode::SetLocal(pos))
            } else if let Some((pos, _, mutability)) = self
                .resolve_upvalue(name.lexeme)
                .inspect_err(|err| self.error_at(name, err))
                .ok()
                .flatten()
            {
                if can_assign && mutability == Mutability::Immutable {
                    self.error_at(name, "Cannot assign to constant variable.");
                }
                (OpCode::GetUpvalue(pos), OpCode::SetUpvalue(pos))
            } else {
                if can_assign && self.const_globals.contains(name.lexeme) {
                    self.error_at(name, "Cannot assign to constant variable.");
                }
                let pos = self.identifier_constant(name.lexeme) as u8;
                (OpCode::GetGlobal(pos), OpCode::SetGlobal(pos))
            };

        if can_assign {
            self.emit(set_op);
        } else {
            self.emit(get_op);
        }
        Ok(())
    }

    // Resolve a local variable by name, return its index and depth.
    fn resolve_local(&mut self, name: &str) -> Option<(u8, isize, Mutability)> {
        (0..self.local_count)
            .rev()
            .find(|&i| self.locals[i].name.lexeme == name)
            .map(|i| (i as u8, self.locals[i].depth, self.locals[i].mutability))
    }

    fn resolve_upvalue(
        &mut self,
        name: &str,
    ) -> Result<Option<(u8, isize, Mutability)>, &'static str> {
        if let Some((index, depth, mutability)) = self
            .enclosing
            .as_mut()
            .and_then(|enclosing| enclosing.resolve_local(name))
        {
            if let Some(enclosing) = self.enclosing.as_mut() {
                // When resolving an identifier, if we end up creating an upvalue for
                // a local variable, we mark it as captured.
                enclosing.locals[index as usize].is_captured = true;
            }
            let index = self.add_upvalue(index as usize, true)?;
            return Ok(Some((index as u8, depth, mutability)));
        } else if let Some((index, depth, mutability)) = self
            .enclosing
            .as_mut()
            .and_then(|enclosing| enclosing.resolve_upvalue(name).ok())
            .flatten()
        {
            let index = self.add_upvalue(index as usize, false)?;
            return Ok(Some((index as u8, depth, mutability)));
        }

        Ok(None)
    }

    fn add_upvalue(&mut self, index: usize, is_local: bool) -> Result<usize, &'static str> {
        let upvalue_index = self.function.upvalues.len();

        // before we add a new upvalue, we first check to see if the function
        // already has an upvalue that closes over that variable.
        if let Some(i) = self
            .function
            .upvalues
            .iter()
            .position(|u| u.index == index && u.is_local == is_local)
        {
            return Ok(i);
        }

        if self.function.upvalues.len() == MAX_LOCALS {
            return Err("Too many closure variables in function.");
        }

        self.function.upvalues.push(Upvalue { index, is_local });
        // println!("add upvalue to {upvalue_index} of {:?}", Upvalue { index, is_local });
        Ok(upvalue_index)
    }

    // Scope management methods
    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.scope_depth -= 1;
        let mut pop_count = 0;

        while self.local_count > 0 && self.locals[self.local_count - 1].depth > self.scope_depth {
            if self.locals[self.local_count - 1].is_captured {
                // Must still handle captured variables one at a time
                if pop_count > 0 {
                    self.emit(OpCode::Pop(pop_count));
                    pop_count = 0;
                }
                // Whenever the compiler reaches the end of a block, it discards all local
                // variables in that block and emits an OpCode::CloseUpvalue for each local variable
                self.emit(OpCode::CloseUpvalue);
            } else {
                pop_count += 1;
            }
            self.local_count -= 1;
        }

        if pop_count > 0 {
            self.emit(OpCode::Pop(pop_count));
        }
    }

    // Constants and identifiers
    fn make_constant(&mut self, value: Value<'gc>) -> usize {
        let constant = self.function.add_constant(value);
        if constant > u8::MAX as usize {
            self.error_at_value(value, "Too many constants in one chunk.");
            0
        } else {
            constant
        }
    }

    fn identifier_constant(&mut self, name: &str) -> usize {
        let s = self.ctx.intern(name.as_bytes());
        self.make_constant(Value::from(s))
    }

    fn declare_variable(&mut self, name: Token<'gc>, mutability: Mutability) {
        if self.scope_depth == 0 {
            return;
        }

        for i in (0..self.local_count).rev() {
            let local = &self.locals[i];
            if local.depth != UNINITIALIZED_LOCAL_DEPTH && local.depth < self.scope_depth {
                // Stop when we reach an outer scope
                break;
            }
            if local.name.lexeme == name.lexeme {
                self.error_at(name, "Already a variable with this name in this scope.");
                // return;
            }
        }

        self.add_local(name, mutability);
    }

    fn add_local(&mut self, name: Token<'gc>, mutability: Mutability) -> usize {
        if self.local_count == MAX_LOCALS {
            self.error_at(name, "Too many local variables in function.");
            return 0;
        }

        self.locals[self.local_count] = Local {
            name,
            depth: UNINITIALIZED_LOCAL_DEPTH, // Mark as uninitialized
            is_captured: false,
            mutability,
        };
        let slot = self.local_count;
        self.local_count += 1;
        slot // Return the slot index
    }

    fn mark_initialized(&mut self) {
        if self.scope_depth == 0 {
            return;
        }
        self.locals[self.local_count - 1].depth = self.scope_depth;
    }

    fn error(&mut self, message: &str) {
        if self.had_error {
            return;
        }
        self.had_error = true;
        eprintln!("[line {}] Error: {}", self.current_line, message);
    }

    fn error_at_value(&mut self, value: Value<'gc>, message: &str) {
        if self.had_error {
            return;
        }
        self.had_error = true;
        eprintln!(
            "[line {}] Error at '{}': {}",
            self.current_line, value, message
        );
    }

    fn error_at(&mut self, token: Token<'gc>, message: &str) {
        if self.had_error {
            return;
        }
        eprint!("[line {}] Error", token.line);
        if token.kind == TokenType::Eof {
            eprint!(" at end");
        } else if token.kind == TokenType::Invalid {
            // Do nothing.
        } else {
            eprint!(" at '{}'", token.lexeme);
        }
        eprintln!(": {message}");
        self.had_error = true;
    }
}
