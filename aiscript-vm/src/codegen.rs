use crate::{
    ast::{Expr, LiteralValue, Program, Stmt},
    object::{Function, FunctionType, Upvalue},
    parser::Parser,
    scanner::{Token, TokenType},
    vm::{Context, VmError},
    OpCode, Value,
};
use gc_arena::Gc;

const MAX_LOCALS: usize = u8::MAX as usize + 1;
const UNINITIALIZED_LOCAL_DEPTH: isize = -1;

#[derive(Debug, Clone, Default)]
struct Local<'gc> {
    name: Token<'gc>,
    depth: isize,
    is_captured: bool,
}

pub struct CodeGen<'gc> {
    ctx: Context<'gc>,
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

    pub fn generate(program: Program<'gc>, ctx: Context<'gc>) -> Result<Function<'gc>, VmError> {
        let mut generator = Self::new(ctx, FunctionType::Script, "");

        for stmt in program.statements {
            generator.generate_stmt(&stmt)?;
        }

        generator.emit_return();

        if generator.had_error {
            Err(VmError::CompileError)
        } else {
            Ok(generator.function)
        }
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
            Stmt::While {
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
                params,
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

                self.generate_function(name.lexeme, params, body, fn_type)?;

                if self.scope_depth == 0 {
                    let global = self.identifier_constant(name.lexeme);
                    self.emit(OpCode::DefineGlobal(global as u8));
                }
            }
            Stmt::Return { value, .. } => {
                if let Some(expr) = value {
                    if self.fn_type == FunctionType::Initializer {
                        self.error("Can't return a value from an initializer.");
                    }
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
                        params,
                        body,
                        ..
                    } = method
                    {
                        let fn_type = if method_name.lexeme == "init" {
                            FunctionType::Initializer
                        } else {
                            FunctionType::Method
                        };
                        self.generate_function(method_name.lexeme, params, body, fn_type)?;
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
        }
        Ok(())
    }

    fn generate_expr(&mut self, expr: &Expr<'gc>) -> Result<(), VmError> {
        self.current_line = expr.line();
        match expr {
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
                LiteralValue::Number(n) => self.emit_constant(Value::from(*n)),
                LiteralValue::String(s) => self.emit_constant(Value::from(*s)),
                LiteralValue::Boolean(true) => self.emit(OpCode::True),
                LiteralValue::Boolean(false) => self.emit(OpCode::False),
                LiteralValue::Nil => self.emit(OpCode::Nil),
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
                self.named_variable(name, true)?;
            }
            Expr::Assign { name, value, .. } => {
                self.generate_expr(value)?;
                let name_constant = self.identifier_constant(name.lexeme);
                self.emit(OpCode::SetGlobal(name_constant as u8));
            }
            Expr::Call {
                callee, arguments, ..
            } => {
                self.generate_expr(callee)?;
                for arg in arguments {
                    self.generate_expr(arg)?;
                }
                self.emit(OpCode::Call(arguments.len() as u8));
            }
            Expr::Invoke {
                object,
                method,
                arguments,
                ..
            } => {
                self.generate_expr(object)?;
                let method_constant = self.identifier_constant(method.lexeme);
                for arg in arguments {
                    self.generate_expr(arg)?;
                }
                self.emit(OpCode::Invoke(method_constant as u8, arguments.len() as u8));
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
            Expr::Super {
                method, arguments, ..
            } => {
                // Get the receiver ('this')
                self.emit(OpCode::GetLocal(0));

                // Generate arguments
                for arg in arguments {
                    self.generate_expr(arg)?;
                }

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
                method, arguments, ..
            } => {
                // Get this instance
                self.emit(OpCode::GetLocal(0));

                // Generate arguments
                for arg in arguments {
                    self.generate_expr(arg)?;
                }

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

    // Helper methods for emitting bytecode and managing functions will continue in the next part...
}

impl<'gc> CodeGen<'gc> {
    fn generate_function(
        &mut self,
        name: &str,
        params: &[Token<'gc>],
        body: &[Stmt<'gc>],
        fn_type: FunctionType,
    ) -> Result<(), VmError> {
        let compiler = Self::new(self.ctx, fn_type, name);

        // Create a new compiler taking ownership of current one
        let enclosing = std::mem::replace(self, *compiler);
        self.enclosing = Some(Box::new(enclosing));

        self.begin_scope();

        // Compile parameters
        self.function.arity = params.len() as u8;
        for param in params {
            self.declare_variable(*param);
            self.mark_initialized();
        }

        // Compile function body
        for stmt in body {
            self.generate_stmt(stmt)?;
        }

        self.emit_return();
        let function = self.function.clone();

        // Restore the original compiler
        if self.had_error {
            return Err(VmError::CompileError);
        }
        if let Some(enclosing) = self.enclosing.take() {
            *self = *enclosing;
        }

        let constant = self.make_constant(Value::from(Gc::new(&self.ctx, function)));
        self.emit(OpCode::Closure(constant as u8));

        Ok(())
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
                self.error("Can't read local variable in its own initializer.");
            }
            (OpCode::GetLocal(pos), OpCode::SetLocal(pos))
        } else if let Some((pos, _)) = self
            .resolve_upvalue(name.lexeme)
            .inspect_err(|err| self.error(err))
            .ok()
            .flatten()
        {
            (OpCode::GetUpvalue(pos), OpCode::SetUpvalue(pos))
        } else {
            let pos = self.identifier_constant(name.lexeme) as u8;
            (OpCode::GetGlobal(pos), OpCode::SetGlobal(pos))
        };

        if can_assign && matches!(name.kind, TokenType::Equal) {
            self.emit(set_op);
        } else {
            self.emit(get_op);
        }
        Ok(())
    }

    fn resolve_local(&self, name: &str) -> Option<(u8, isize)> {
        for i in (0..self.local_count).rev() {
            let local = &self.locals[i];
            if local.name.lexeme == name {
                return Some((i as u8, local.depth));
            }
        }
        None
    }

    fn resolve_upvalue(&mut self, name: &str) -> Result<Option<(u8, isize)>, &'static str> {
        if let Some(enclosing) = self.enclosing.as_mut() {
            if let Some((index, depth)) = enclosing.resolve_local(name) {
                enclosing.locals[index as usize].is_captured = true;
                let upvalue_index = self.add_upvalue(index as usize, true)?;
                return Ok(Some((upvalue_index as u8, depth)));
            }

            if let Some((index, depth)) = enclosing.resolve_upvalue(name)? {
                let upvalue_index = self.add_upvalue(index as usize, false)?;
                return Ok(Some((upvalue_index as u8, depth)));
            }
        }
        Ok(None)
    }

    fn add_upvalue(&mut self, index: usize, is_local: bool) -> Result<usize, &'static str> {
        let upvalue_count = self.function.upvalues.len();

        // Check if we already have this upvalue
        for i in 0..upvalue_count {
            let upvalue = &self.function.upvalues[i];
            if upvalue.index == index && upvalue.is_local == is_local {
                return Ok(i);
            }
        }

        if upvalue_count >= MAX_LOCALS {
            return Err("Too many closure variables in function.");
        }

        self.function.upvalues.push(Upvalue { index, is_local });
        Ok(upvalue_count)
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
            self.error("Too many constants in one chunk.");
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

        // Check for variable redeclaration in current scope
        for i in (0..self.local_count).rev() {
            let local = &self.locals[i];
            if local.depth != -1 && local.depth < self.scope_depth {
                break;
            }
            if local.name.lexeme == name.lexeme {
                self.error("Already a variable with this name in this scope.");
                return;
            }
        }

        if self.local_count == MAX_LOCALS {
            self.error("Too many local variables in function.");
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

    // Error handling
    fn error(&mut self, message: &str) {
        self.had_error = true;
        eprintln!("[line {}] Error: {}", self.current_line, message);
    }
}

pub fn compile<'gc>(ctx: Context<'gc>, source: &'gc str) -> Result<Function<'gc>, VmError> {
    // Step 1: Parse source into AST
    let mut parser = Parser::new(ctx, source);
    let program = parser.parse()?;
    // println!("AST: {}", program);
    // Step 2: Generate bytecode from AST
    CodeGen::generate(program, ctx)
}
