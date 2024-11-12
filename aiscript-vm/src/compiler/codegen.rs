use std::{
    collections::HashMap,
    mem,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{
    ast::{Expr, FnDef, Literal, Parameter, Program, Stmt},
    lexer::{Token, TokenType},
    ty::{Type, TypeResolver},
};
use crate::{
    ai::Agent,
    object::{Function, FunctionType, Upvalue},
    vm::{Context, VmError},
    OpCode, Value,
};
use gc_arena::Gc;
use indexmap::IndexMap;

const MAX_LOCALS: usize = u8::MAX as usize + 1;
const UNINITIALIZED_LOCAL_DEPTH: isize = -1;
static CHUNK_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Default)]
struct Local<'gc> {
    name: Token<'gc>,
    depth: isize,
    is_captured: bool,
}

pub struct CodeGen<'gc> {
    ctx: Context<'gc>,
    chunks: HashMap<usize, Function<'gc>>,
    // <function mangled name, chunk_id>
    named_id_map: HashMap<String, FnDef>,
    type_resolver: TypeResolver<'gc>,
    function: Function<'gc>,
    fn_type: FunctionType,
    locals: [Local<'gc>; MAX_LOCALS],
    local_count: usize,
    scope_depth: isize,
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
                if i == 0 {
                    let name = if fn_type != FunctionType::Function {
                        Token::new(TokenType::This, "this", 0)
                    } else {
                        Token::default()
                    };
                    Local {
                        name,
                        depth: 0,
                        is_captured: false,
                    }
                } else {
                    Local::default()
                }
            }),
            local_count: 1,
            scope_depth: 0,
            enclosing: None,
            current_line: 0,
            had_error: false,
        });

        generator
    }

    pub fn generate(
        program: Program<'gc>,
        ctx: Context<'gc>,
    ) -> Result<HashMap<usize, Function<'gc>>, VmError> {
        let mut generator = Self::new(ctx, FunctionType::Script, "script");

        for stmt in &program.statements {
            generator.declare_classes(stmt)?;
        }

        for stmt in &program.statements {
            generator.declare_functions(stmt)?;
        }

        for stmt in program.statements {
            generator.generate_stmt(&stmt)?;
        }

        generator.emit_return();

        if generator.had_error {
            Err(VmError::CompileError)
        } else {
            let function = mem::take(&mut generator.function);
            generator.chunks.insert(0, function);
            Ok(generator.chunks)
        }
    }

    fn declare_classes(&mut self, stmt: &Stmt<'gc>) -> Result<(), VmError> {
        if let Stmt::Class { name, .. } = stmt {
            self.type_resolver
                .register_type(name.lexeme, Type::Class(*name));
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
            Stmt::Function {
                name,
                mangled_name,
                body,
                doc,
                params,
                ..
            } => {
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
            Stmt::Class { methods, .. } => {
                for methods in methods {
                    self.declare_functions(methods)?;
                }
            }
            Stmt::Agent { tools, .. } => {
                for tool in tools {
                    self.declare_functions(tool)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn generate_stmt(&mut self, stmt: &Stmt<'gc>) -> Result<(), VmError> {
        self.current_line = stmt.line();
        match stmt {
            Stmt::Expression { expression, .. } => {
                self.generate_expr(expression)?;
                self.emit(OpCode::Pop);
            }
            Stmt::Print { expression, .. } => {
                self.generate_expr(expression)?;
                self.emit(OpCode::Print);
            }
            Stmt::Let {
                name, initializer, ..
            } => {
                self.declare_variable(*name);
                if let Some(init) = initializer {
                    self.generate_expr(init)?;
                } else {
                    self.emit(OpCode::Nil);
                }
                if self.scope_depth > 0 {
                    self.mark_initialized();
                } else {
                    let global = self.identifier_constant(name.lexeme);
                    self.emit(OpCode::DefineGlobal(global as u8));
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
                self.emit(OpCode::Pop);
                self.generate_stmt(then_branch)?;

                let else_jump = self.emit_jump(OpCode::Jump(0));
                self.patch_jump(then_jump);
                self.emit(OpCode::Pop);

                if let Some(else_branch) = else_branch {
                    self.generate_stmt(else_branch)?;
                }
                self.patch_jump(else_jump);
            }
            Stmt::Loop {
                condition, body, ..
            } => {
                let loop_start = self.function.code_size();
                self.generate_expr(condition)?;

                let exit_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                self.emit(OpCode::Pop);
                self.generate_stmt(body)?;
                self.emit_loop(loop_start);

                self.patch_jump(exit_jump);
                self.emit(OpCode::Pop);
            }
            Stmt::Function {
                name,
                mangled_name,
                params,
                return_type,
                body,
                is_ai,
                ..
            } => {
                let fn_type = if *is_ai {
                    FunctionType::AiFunction
                } else {
                    FunctionType::Function
                };

                self.declare_variable(*name);
                if self.scope_depth > 0 {
                    self.mark_initialized();
                }

                self.generate_function(
                    name.lexeme,
                    mangled_name.to_owned(),
                    params,
                    return_type,
                    body,
                    fn_type,
                )?;

                if self.scope_depth == 0 {
                    let global = self.identifier_constant(name.lexeme);
                    self.emit(OpCode::DefineGlobal(global as u8));
                }
            }
            Stmt::Return { value, .. } => {
                if let Some(expr) = value {
                    self.generate_expr(expr)?;
                    self.emit(OpCode::Return);
                } else {
                    if self.fn_type == FunctionType::Initializer {
                        self.emit(OpCode::GetLocal(0));
                    } else {
                        self.emit(OpCode::Nil);
                    }
                    self.emit(OpCode::Return);
                }
            }
            Stmt::Class {
                name,
                superclass,
                methods,
                ..
            } => {
                // Emit class declaration
                let name_constant = self.identifier_constant(name.lexeme);
                self.emit(OpCode::Class(name_constant as u8));
                self.emit(OpCode::DefineGlobal(name_constant as u8));

                // Handle inheritance
                if let Some(superclass) = superclass {
                    // Begin a new scope for 'super'
                    self.begin_scope();

                    // First get the superclass
                    self.generate_expr(superclass)?;

                    // Create local variable 'super'
                    let super_token = Token::new(TokenType::Super, "super", name.line);
                    self.declare_variable(super_token);
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
                    if let Stmt::Function {
                        name: method_name,
                        mangled_name,
                        params,
                        return_type,
                        body,
                        ..
                    } = method
                    {
                        let fn_type = if method_name.lexeme == "init" {
                            FunctionType::Initializer
                        } else {
                            FunctionType::Method
                        };
                        self.generate_function(
                            method_name.lexeme,
                            mangled_name.to_owned(),
                            params,
                            return_type,
                            body,
                            fn_type,
                        )?;
                        let method_constant = self.identifier_constant(method_name.lexeme);
                        self.emit(OpCode::Method(method_constant as u8));
                    }
                }

                // Once we’ve reached the end of the methods, we no longer need
                // the class and tell the VM to pop it off the stack.
                self.emit(OpCode::Pop);

                // Close the scope created for 'super' if there was inheritance
                if superclass.is_some() {
                    self.end_scope();
                }
            }
            Stmt::Agent {
                name,
                mangled_name,
                fields,
                tools,
                ..
            } => {
                // Emit agent declaration
                let agent_name = self.ctx.intern(name.lexeme.as_bytes());
                let mut agent = Agent::new(&self.ctx, agent_name)
                    .parse_instructions(fields)
                    .parse_model(fields)
                    .parse_tools(fields, |name| {
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

                for tool in tools {
                    if let Stmt::Function {
                        name,
                        mangled_name,
                        doc,
                        params,
                        return_type,
                        body,
                        ..
                    } = tool
                    {
                        let fn_type = FunctionType::Tool;
                        let chunk_id = self.generate_function(
                            name.lexeme,
                            mangled_name.to_owned(),
                            params,
                            return_type,
                            body,
                            fn_type,
                        )?;
                        if agent
                            .tools
                            .insert(name.lexeme.to_string(), FnDef::new(chunk_id, doc, params))
                            .is_some()
                        {
                            self.error_at(*name, &format!("Duplicate tool name: {}", name.lexeme));
                        }
                    }
                }
                let agent = Gc::new(&self.ctx, agent);
                let agent_constant = self.make_constant(Value::from(agent));
                self.emit(OpCode::Agent(agent_constant as u8));
                let name_constant = self.identifier_constant(name.lexeme);
                self.emit(OpCode::DefineGlobal(name_constant as u8));
                // self.emit(OpCode::Pop);
            }
        }
        Ok(())
    }

    fn generate_expr(&mut self, expr: &Expr<'gc>) -> Result<(), VmError> {
        self.current_line = expr.line();
        match expr {
            Expr::Array { .. } => {}
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
                    TokenType::Slash => self.emit(OpCode::Divide),
                    TokenType::BangEqual => {
                        self.emit(OpCode::Equal);
                        self.emit(OpCode::Not);
                    }
                    TokenType::EqualEqual => self.emit(OpCode::Equal),
                    TokenType::Greater => self.emit(OpCode::Greater),
                    TokenType::GreaterEqual => {
                        self.emit(OpCode::Less);
                        self.emit(OpCode::Not);
                    }
                    TokenType::Less => self.emit(OpCode::Less),
                    TokenType::LessEqual => {
                        self.emit(OpCode::Greater);
                        self.emit(OpCode::Not);
                    }
                    _ => return Err(VmError::CompileError),
                }
            }
            Expr::Grouping { expression, .. } => {
                self.generate_expr(expression)?;
            }
            Expr::Literal { value, .. } => match value {
                Literal::Number(n) => self.emit_constant(Value::from(*n)),
                Literal::String(s) => self.emit_constant(Value::from(*s)),
                Literal::Boolean(true) => self.emit(OpCode::True),
                Literal::Boolean(false) => self.emit(OpCode::False),
                Literal::Nil => self.emit(OpCode::Nil),
            },
            Expr::Unary {
                operator, right, ..
            } => {
                self.generate_expr(right)?;
                match operator.kind {
                    TokenType::Minus => self.emit(OpCode::Negate),
                    TokenType::Bang => self.emit(OpCode::Not),
                    _ => return Err(VmError::CompileError),
                }
            }
            Expr::Variable { name, .. } => {
                self.named_variable(name, false)?;
            }
            Expr::Assign { name, value, .. } => {
                self.generate_expr(value)?;
                self.named_variable(name, true)?;
            }
            Expr::Call {
                callee,
                arguments,
                keyword_args,
                ..
            } => {
                self.generate_expr(callee)?;
                for arg in arguments {
                    self.generate_expr(arg)?;
                }
                // Create and emit constants for the keyword names
                self.generate_keyword_args(keyword_args)?;
                self.emit(OpCode::Call(
                    arguments.len() as u8,
                    keyword_args.len() as u8,
                ));
            }
            Expr::Invoke {
                object,
                method,
                arguments,
                keyword_args,
                ..
            } => {
                self.generate_expr(object)?;
                let method_constant = self.identifier_constant(method.lexeme);
                for arg in arguments {
                    self.generate_expr(arg)?;
                }
                self.generate_keyword_args(keyword_args)?;
                self.emit(OpCode::Invoke(
                    method_constant as u8,
                    arguments.len() as u8,
                    keyword_args.len() as u8,
                ));
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
            Expr::This { .. } => {
                self.named_variable(&Token::new(TokenType::This, "this", 0), false)?;
            }
            Expr::Super { method, .. } => {
                // Get the receiver ('this')
                self.emit(OpCode::GetLocal(0));

                // Look up 'super' in upvalues
                let method_constant = self.identifier_constant(method.lexeme);
                if let Some((pos, _)) = self
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

                // Generate arguments
                for arg in arguments {
                    self.generate_expr(arg)?;
                }
                self.generate_keyword_args(keyword_args)?;

                // Get superclass and invoke method
                if let Some((pos, _)) = self
                    .resolve_upvalue("super")
                    .map_err(|e| VmError::RuntimeError(e.into()))?
                {
                    self.emit(OpCode::GetUpvalue(pos));
                } else {
                    return Err(VmError::RuntimeError("Unable to resolve 'super'".into()));
                }

                let method_constant = self.identifier_constant(method.lexeme);
                self.emit(OpCode::SuperInvoke(
                    method_constant as u8,
                    arguments.len() as u8,
                    keyword_args.len() as u8,
                ));
            }
            Expr::And { left, right, .. } => {
                self.generate_expr(left)?;
                let end_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                self.emit(OpCode::Pop);
                self.generate_expr(right)?;
                self.patch_jump(end_jump);
            }
            Expr::Or { left, right, .. } => {
                self.generate_expr(left)?;
                let else_jump = self.emit_jump(OpCode::JumpIfFalse(0));
                let end_jump = self.emit_jump(OpCode::Jump(0));

                self.patch_jump(else_jump);
                self.emit(OpCode::Pop);
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
        keyword_args: &HashMap<String, Expr<'gc>>,
    ) -> Result<(), VmError> {
        for (name, value) in keyword_args {
            let name_constant = self.identifier_constant(name);
            self.emit(OpCode::Constant(name_constant as u8));
            self.generate_expr(value)?;
        }
        Ok(())
    }
}

impl<'gc> CodeGen<'gc> {
    fn generate_function(
        &mut self,
        name: &'gc str,
        mangle_name: String,
        params: &IndexMap<Token<'gc>, Parameter<'gc>>,
        return_type: &Option<Token<'gc>>,
        body: &[Stmt<'gc>],
        fn_type: FunctionType,
    ) -> Result<usize, VmError> {
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
            self.declare_variable(param.name);
            self.mark_initialized();

            let name = self.ctx.intern(param.name.lexeme.as_bytes());
            // Store default value if present
            if let Some(Expr::Literal { value, .. }) = &param.default_value {
                self.function
                    .params
                    .insert(name, (index as u8, Value::from(value)));
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
                .get(&mangle_name)
                .map(|n| n.chunk_id)
                .unwrap();
            // TODO: Duplicate function name?
            self.chunks.insert(chunk_id, function);
            enclosing.named_id_map = mem::take(&mut self.named_id_map);
            let chunks = mem::take(&mut self.chunks);
            *self = *enclosing;
            self.chunks.extend(chunks);
            self.emit(OpCode::Closure(chunk_id as u8));
        }
        Ok(chunk_id)
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
        if self.fn_type == FunctionType::Initializer {
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
    fn named_variable(&mut self, name: &Token<'gc>, can_assign: bool) -> Result<(), VmError> {
        let (get_op, set_op) = if let Some((pos, depth)) = self.resolve_local(name.lexeme) {
            if depth == UNINITIALIZED_LOCAL_DEPTH {
                self.error_at(*name, "Can't read local variable in its own initializer.");
            }
            (OpCode::GetLocal(pos), OpCode::SetLocal(pos))
        } else if let Some((pos, _)) = self
            .resolve_upvalue(name.lexeme)
            .inspect_err(|err| self.error_at(*name, err))
            .ok()
            .flatten()
        {
            (OpCode::GetUpvalue(pos), OpCode::SetUpvalue(pos))
        } else {
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
    fn resolve_local(&mut self, name: &str) -> Option<(u8, isize)> {
        (0..self.local_count)
            .rev()
            .find(|&i| self.locals[i].name.lexeme == name)
            .map(|i| (i as u8, self.locals[i].depth))
    }

    fn resolve_upvalue(&mut self, name: &str) -> Result<Option<(u8, isize)>, &'static str> {
        if let Some((index, depth)) = self
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
            return Ok(Some((index as u8, depth)));
        } else if let Some((index, depth)) = self
            .enclosing
            .as_mut()
            .and_then(|enclosing| enclosing.resolve_upvalue(name).ok())
            .flatten()
        {
            let index = self.add_upvalue(index as usize, false)?;
            return Ok(Some((index as u8, depth)));
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

        while self.local_count > 0 && self.locals[self.local_count - 1].depth > self.scope_depth {
            if self.locals[self.local_count - 1].is_captured {
                self.emit(OpCode::CloseUpvalue);
            } else {
                self.emit(OpCode::Pop);
            }
            self.local_count -= 1;
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

    fn declare_variable(&mut self, name: Token<'gc>) {
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

        self.add_local(name);
    }

    fn add_local(&mut self, name: Token<'gc>) {
        if self.local_count == MAX_LOCALS {
            self.error_at(name, "Too many local variables in function.");
            return;
        }

        self.locals[self.local_count] = Local {
            name,
            depth: UNINITIALIZED_LOCAL_DEPTH, // Mark as uninitialized
            is_captured: false,
        };
        self.local_count += 1;
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
        } else if token.kind == TokenType::Error {
            // Do nothing.
        } else {
            eprint!(" at '{}'", token.lexeme);
        }
        eprintln!(": {message}");
        self.had_error = true;
    }
}
