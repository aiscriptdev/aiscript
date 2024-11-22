use std::{
    array,
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    hash::BuildHasherDefault,
    mem,
};

use ahash::AHasher;
use gc_arena::{
    lock::{GcRefLock, RefLock},
    Collect, Collection, Gc, Mutation,
};

use crate::{
    ai,
    ast::{ChunkId, Visibility},
    builtins,
    module::{ModuleKind, ModuleManager, ModuleSource},
    object::{BoundMethod, Class, Closure, Function, Instance, Object, Upvalue, UpvalueObj},
    string::{InternedString, InternedStringSet},
    NativeFn, OpCode, ReturnValue, Value,
};

use super::{fuel::Fuel, Context, VmError};

type Table<'gc> = HashMap<InternedString<'gc>, Value<'gc>, BuildHasherDefault<AHasher>>;

const FRAME_MAX_SIZE: usize = 64;
// const STACK_MAX_SIZE: usize = FRAME_MAX_SIZE * (u8::MAX as usize + 1);
#[cfg(not(test))]
const STACK_MAX_SIZE: usize = 4096; // Temporary reduce the stack size due to tokio thread stack size limit
#[cfg(test)]
const STACK_MAX_SIZE: usize = 128;

static NUMBER_OPERATOR_ERROR: &str = "Operands must be numbers.";

macro_rules! binary_op {
    ($self:expr, $op:tt) => {{
        debug_assert!($self.stack_top >= 2, "Stack underflow in binary op");
        let b = unsafe { $self.stack.get_unchecked($self.stack_top - 1) }
            .as_number()
            .map_err(|_| $self.runtime_error(NUMBER_OPERATOR_ERROR.into()))?;
        let a = unsafe { $self.stack.get_unchecked($self.stack_top - 2) }
            .as_number()
            .map_err(|_| $self.runtime_error(NUMBER_OPERATOR_ERROR.into()))?;
        $self.stack_top -= 2;
        $self.push_stack((a $op b).into());
    }};
}

#[derive(Debug, Clone, Collect)]
#[collect(no_drop)]
struct CallFrame<'gc> {
    closure: Gc<'gc, Closure<'gc>>,
    // When we return from a function, the VM will
    // jump to the ip of the caller’s CallFrame and resume from there.
    ip: usize,
    // slot_start field points into the VM’s value stack
    // at the first slot that this function can use
    slot_start: usize,
}

impl<'gc> CallFrame<'gc> {
    fn next_opcode(&mut self) -> OpCode {
        let byte = self.closure.function[self.ip];
        self.ip += 1;
        byte
    }

    fn read_constant(&mut self, byte: u8) -> Value<'gc> {
        self.closure.function.read_constant(byte)
    }

    #[allow(unused)]
    fn disassemble(&self) {
        self.closure
            .function
            .disassemble(self.closure.function.name.unwrap().display_lossy());
    }

    #[allow(unused)]
    fn disassemble_instruction(&self, offset: usize) {
        self.closure.function.disassemble_instruction(offset);
    }
}

pub struct State<'gc> {
    mc: &'gc Mutation<'gc>,
    pub(super) chunks: BTreeMap<ChunkId, Gc<'gc, Function<'gc>>>,
    frames: Vec<CallFrame<'gc>>,
    frame_count: usize,
    stack: [Value<'gc>; STACK_MAX_SIZE],
    stack_top: usize,
    pub(super) strings: InternedStringSet<'gc>,
    pub(super) globals: Table<'gc>,
    open_upvalues: Option<GcRefLock<'gc, UpvalueObj<'gc>>>,
    pub module_manager: ModuleManager<'gc>,
    current_module: Option<InternedString<'gc>>,
}

unsafe impl<'gc> Collect for State<'gc> {
    fn needs_trace() -> bool
    where
        Self: Sized,
    {
        true
    }

    fn trace(&self, cc: &Collection) {
        self.frames.trace(cc);
        self.frame_count.trace(cc);
        self.stack.trace(cc);
        self.stack_top.trace(cc);
        self.strings.trace(cc);
        self.globals.trace(cc);
        self.open_upvalues.trace(cc);
        self.module_manager.trace(cc);
        self.current_module.trace(cc);
    }
}

impl<'gc> State<'gc> {
    pub(super) fn new(mc: &'gc Mutation<'gc>) -> Self {
        State {
            mc,
            chunks: BTreeMap::new(),
            frames: Vec::with_capacity(FRAME_MAX_SIZE),
            frame_count: 0,
            stack: array::from_fn(|_| Value::Nil),
            stack_top: 0,
            strings: InternedStringSet::new(mc),
            globals: HashMap::default(),
            open_upvalues: None,
            module_manager: ModuleManager::new(),
            current_module: None,
        }
    }
    pub fn import_module(&mut self, path: InternedString<'gc>) -> Result<(), VmError> {
        // Get the simple name (last component) from the path
        let simple_name = path.to_str().unwrap().split('.').last().unwrap();
        let simple_name = self.intern(simple_name.as_bytes());

        // Check if simple name is already used
        if self.globals.contains_key(&simple_name) {
            return Err(VmError::RuntimeError(format!(
                "Name '{}' is already in use",
                simple_name
            )));
        }

        // Get module source
        let module_source = self.module_manager.get_or_load_module(path)?;

        match module_source {
            ModuleSource::Cached => {
                // For any module (std or script), just bind it to its simple name
                self.globals.insert(simple_name, Value::Module(path));
                Ok(())
            }
            ModuleSource::New {
                source,
                path: module_path,
            } => {
                let prev_module = self.current_module.replace(path);
                let prev_globals = mem::take(&mut self.globals);

                let module = ModuleKind::Script {
                    name: path,
                    exports: HashMap::default(),
                    globals: HashMap::default(),
                    path: module_path,
                };

                self.module_manager.register_script_module(path, module);

                let context = Context {
                    mutation: self.mc,
                    strings: self.strings,
                };

                let source: &'static str = Box::leak(source.into_boxed_str());
                let chunks = crate::compiler::compile(context, source)?;

                let imported_script_chunk_id = chunks.keys().last().copied().unwrap();
                self.chunks.extend(chunks);
                self.eval_function(imported_script_chunk_id, &[])?;

                if let Some(ModuleKind::Script {
                    ref mut globals, ..
                }) = self.module_manager.modules.get_mut(&path)
                {
                    *globals = mem::replace(&mut self.globals, prev_globals);
                }

                // Add the module to globals with its simple name
                self.globals.insert(simple_name, Value::Module(path));

                self.current_module = prev_module;
                Ok(())
            }
        }
    }
    pub fn get_global(&self, name: InternedString<'gc>) -> Option<Value<'gc>> {
        // First check if it's a module name
        if let Some(module) = self.module_manager.get_module(name) {
            return Some(Value::Module(module.name()));
        }

        // Then check current globals scope
        if let Some(value) = self.globals.get(&name).copied() {
            return Some(value);
        }

        // Finally check current module's globals if we're in a module
        if let Some(current_module) = self.current_module {
            if let Some(ModuleKind::Script { globals, .. }) =
                self.module_manager.modules.get(&current_module)
            {
                if let Some(value) = globals.get(&name).copied() {
                    return Some(value);
                }
            }
        }

        None
    }

    pub fn gc_ref<T: Collect>(&mut self, value: T) -> GcRefLock<'gc, T> {
        Gc::new(self.mc, RefLock::new(value))
    }

    pub fn intern(&mut self, s: &[u8]) -> InternedString<'gc> {
        self.strings.intern(self.mc, s)
    }

    pub fn intern_static(&mut self, s: &'static str) -> InternedString<'gc> {
        self.strings.intern_static(self.mc, s.as_bytes())
    }

    pub fn get_chunk(&mut self, chunk_id: ChunkId) -> Result<Gc<'gc, Function<'gc>>, VmError> {
        self.chunks.get(&chunk_id).copied().ok_or_else(|| {
            VmError::RuntimeError(format!("Failed to find chunk with id {}", chunk_id))
        })
    }

    // Call function with params
    pub fn call_function(
        &mut self,
        chunk_id: ChunkId,
        params: &[Value<'gc>],
    ) -> Result<(), VmError> {
        let function = self.get_chunk(chunk_id)?;
        #[cfg(feature = "debug")]
        function.disassemble("script");

        let closure = Gc::new(self.mc, Closure::new(self.mc, function));
        self.push_stack(Value::from(closure));
        for param in params {
            self.push_stack(*param);
        }
        self.call(closure, function.arity, 0)
    }
}

impl<'gc> State<'gc> {
    fn runtime_error(&mut self, message: Cow<'static, str>) -> VmError {
        let mut error_message = String::from(message);
        for i in (0..self.frame_count).rev() {
            let frame = &self.frames[i];
            // Break loop if reach the un-initialized callframe.
            // Call Vm::eval_function directly will reach this case,
            // since it never init the root script.
            if frame.ip == 0 {
                break;
            }
            let function = &frame.closure.function;
            error_message.push_str(&format!(
                "\n[line {}] in ",
                function.chunk.line(frame.ip - 1)
            ));
            let name = if let Some(name) = function.name {
                name.to_str().unwrap()
            } else {
                "script"
            };
            error_message.push_str(name);
            error_message.push('\n');
        }
        VmError::RuntimeError(error_message)
    }

    fn current_frame(&mut self) -> &mut CallFrame<'gc> {
        &mut self.frames[self.frame_count - 1]
    }

    // Dispatch the next opcode, stop at the given frame count.
    // When dispatch in step() function, the stop_at_frame_count is 0.
    // When dispatch in eval_function(), the stop_at_frame_count is the frame count before to call eval_function().
    // This is used to exit the frame call after the chunks of that function is finished.
    pub fn dispatch_next(
        &mut self,
        stop_at_frame_count: usize,
    ) -> Result<Option<ReturnValue>, VmError> {
        // Debug stack info
        #[cfg(feature = "debug")]
        self.print_stack();
        let frame = self.current_frame();
        // Disassemble instruction for debug
        #[cfg(feature = "debug")]
        frame.disassemble_instruction(frame.ip);
        match frame.next_opcode() {
            OpCode::Constant(byte) => {
                let constant = frame.read_constant(byte);
                self.push_stack(constant);
            }
            OpCode::Add => match (self.peek(0), self.peek(1)) {
                (Value::Number(_), Value::Number(_)) => {
                    binary_op!(self, +);
                }
                (Value::String(_), Value::String(_))
                | (Value::IoString(_), Value::IoString(_))
                | (Value::String(_), Value::IoString(_))
                | (Value::IoString(_), Value::String(_)) => {
                    let b = self.pop_stack().as_string()?;
                    let a = self.pop_stack().as_string()?;
                    let s = self.intern(format!("{a}{b}").as_bytes());
                    self.push_stack(s.into());
                }
                _ => {
                    return Err(
                        self.runtime_error("Operands must be two numbers or two strings.".into())
                    );
                }
            },
            OpCode::Subtract => {
                binary_op!(self, -);
            }
            OpCode::Multiply => {
                binary_op!(self, *);
            }
            OpCode::Divide => {
                binary_op!(self, /);
            }
            OpCode::Modulo => {
                binary_op!(self, %);
            }
            OpCode::Power => {
                let b = self
                    .pop_stack()
                    .as_number()
                    .map_err(|_| self.runtime_error(NUMBER_OPERATOR_ERROR.into()))?;
                let a = self
                    .pop_stack()
                    .as_number()
                    .map_err(|_| self.runtime_error(NUMBER_OPERATOR_ERROR.into()))?;

                // Use f64's powf method for power operation
                self.push_stack(a.powf(b).into());
            }
            OpCode::Negate => {
                let v = self
                    .pop_stack()
                    .as_number()
                    .map_err(|_| self.runtime_error("Operand must be a number.".into()))?;
                self.push_stack((-v).into());
            }
            OpCode::Return => {
                let frame_slot_start = frame.slot_start;
                let return_value: Value<'_> = self.pop_stack();
                self.close_upvalues(frame_slot_start);
                // Must pop the frame from vec when returning
                self.frames.pop();
                self.frame_count -= 1;
                if self.frame_count == stop_at_frame_count {
                    self.pop_stack();
                    return Ok(Some(return_value.into()));
                }
                self.stack_top = frame_slot_start;
                self.push_stack(return_value);
            }
            OpCode::Nil => self.push_stack(Value::Nil),
            OpCode::Bool(b) => self.push_stack(Value::Boolean(b)),
            OpCode::Not => {
                let v = self.pop_stack().is_falsy();
                self.push_stack((v).into())
            }
            OpCode::Equal => {
                let b = self.pop_stack();
                let a = self.pop_stack();
                self.push_stack(a.equals(&b).into());
            }
            OpCode::NotEqual => {
                let b = self.pop_stack();
                let a = self.pop_stack();
                self.push_stack((!a.equals(&b)).into());
            }
            OpCode::Greater => {
                binary_op!(self, >);
            }
            OpCode::GreaterEqual => {
                binary_op!(self, >=);
            }
            OpCode::Less => {
                binary_op!(self, <);
            }
            OpCode::LessEqual => {
                binary_op!(self, <=);
            }
            OpCode::Print => {
                let value = self.pop_stack();
                println!("{value}");
            }
            OpCode::Pop(count) => {
                self.stack_top = self.stack_top.saturating_sub(count as usize);
            }
            OpCode::DefineGlobal {
                name_constant,
                visibility,
            } => {
                let variable_name = frame.read_constant(name_constant).as_string()?;
                let value = *self.peek(0);

                // Define in global scope with visibility
                self.define_global(variable_name, value, visibility);
                self.pop_stack(); // Pop the value after defining
            }
            OpCode::GetGlobal(byte) => {
                let variable_name = frame.read_constant(byte).as_string()?;
                if let Some(value) = self.get_global(variable_name) {
                    self.push_stack(value);
                } else {
                    return Err(self
                        .runtime_error(format!("Undefined variable '{}'.", variable_name).into()));
                }
            }
            OpCode::SetGlobal(byte) => {
                let varible_name = frame.read_constant(byte).as_string()?;
                #[allow(clippy::map_entry)]
                if self.globals.contains_key(&varible_name) {
                    self.globals.insert(varible_name, *self.peek(0));
                } else {
                    return Err(self
                        .runtime_error(format!("Undefined variable '{}'.", varible_name).into()));
                }
            }
            OpCode::GetLocal(slot) => {
                let value = self.stack[frame.slot_start + slot as usize];
                self.push_stack(value);
            }
            OpCode::SetLocal(slot) => {
                let slot_start = frame.slot_start;
                self.stack[slot_start + slot as usize] = *self.peek(0);
            }
            OpCode::JumpIfFalse(offset) => {
                let is_falsy = self.peek(0).is_falsy();
                // Alwasy jump to the next instruction, do not move this line into if block
                if is_falsy {
                    let frame = self.current_frame();
                    frame.ip += offset as usize;
                }
            }
            OpCode::Jump(offset) => {
                frame.ip += offset as usize;
            }
            OpCode::Loop(offset) => {
                frame.ip -= offset as usize;
            }
            OpCode::Call {
                positional_count,
                keyword_count,
            } => {
                // *2 because each keyword arg has name and value
                // Get the actual function from the correct stack position
                // Need to peek past all args (both positional and keyword) to get to the function
                let arg_slot_count = positional_count + keyword_count * 2;
                let callee = *self.peek(arg_slot_count as usize);
                self.call_value(callee, positional_count, keyword_count)?;
            }
            OpCode::Closure { chunk_id } => {
                let function = self.get_chunk(chunk_id)?;
                let mut closure = Closure::new(self.mc, function);

                closure
                    .function
                    .upvalues
                    .iter()
                    .enumerate()
                    .for_each(|(i, upvalue)| {
                        let frame = self.current_frame();
                        let Upvalue { is_local, index } = *upvalue;
                        if is_local {
                            let slot = frame.slot_start + index;
                            let upvalue = self.capture_upvalue(slot);
                            // println!("function {} capture local: {slot}, {:?}", fn_name, upvalue);
                            closure.upvalues[i] = upvalue;
                        } else {
                            // println!(
                            //     "function {} capture upvalue: {index} {:?}",
                            //     fn_name, &frame.closure.upvalues[index]
                            // );
                            closure.upvalues[i] = frame.closure.upvalues[index];
                        }
                    });

                self.push_stack(Value::from(Gc::new(self.mc, closure)));
            }
            OpCode::GetUpvalue(slot) => {
                let slot = slot as usize;
                let upvalue = frame.closure.upvalues[slot];
                if let Some(closed) = upvalue.borrow().closed {
                    self.push_stack(closed);
                } else {
                    let location = frame.closure.upvalues[slot].borrow().location;
                    let upvalue = self.stack[location];
                    self.push_stack(upvalue);
                }
            }
            OpCode::SetUpvalue(slot) => {
                let slot = slot as usize;
                let mut upvalue = frame.closure.upvalues[slot].borrow_mut(self.mc);
                let stack_position = upvalue.location;
                upvalue.location = slot;

                let value = *self.peek(slot);
                upvalue.closed = Some(value);
                // Also update the stack value
                self.stack[stack_position] = value;
            }
            OpCode::CloseUpvalue => {
                self.close_upvalues(self.stack_top - 1);
                self.pop_stack();
            }
            OpCode::Class(byte) => {
                let name = frame.read_constant(byte).as_string().unwrap();
                self.push_stack(Value::from(Gc::new(
                    self.mc,
                    RefLock::new(Class::new(name)),
                )));
            }
            OpCode::GetProperty(byte) => {
                let name = frame.read_constant(byte).as_string().unwrap();
                match *self.peek(0) {
                    Value::Object(obj) => {
                        // Pop the target object first
                        self.pop_stack();
                        // Default is nil if no key found
                        let value = obj.borrow().fields.get(&name).copied().unwrap_or_default();
                        self.push_stack(value);
                    }
                    Value::Instance(instance) => {
                        if let Some(property) = instance.borrow().fields.get(&name) {
                            self.pop_stack(); // Instance
                            self.push_stack(*property);
                        } else {
                            self.bind_method(instance.borrow().class, name)?;
                        }
                    }
                    Value::Module(module_name) => {
                        if let Some(value) = self.module_manager.get_export(module_name, name) {
                            self.pop_stack(); // Pop module
                            self.push_stack(value);
                        } else {
                            return Err(self.runtime_error(
                                format!(
                                    "Undefined property '{}' in module '{}'",
                                    name, module_name
                                )
                                .into(),
                            ));
                        }
                    }
                    _ => {
                        // Only instances and modules have properties.
                        return Err(self.runtime_error("Only instances have properties.".into()));
                    }
                }
            }
            OpCode::SetProperty(byte) => {
                let value = *self.peek(0);
                match *self.peek(1) {
                    Value::Instance(instantce) => {
                        let frame = self.current_frame();
                        let name = frame.read_constant(byte).as_string().unwrap();
                        instantce.borrow_mut(self.mc).fields.insert(name, value);

                        let value = self.pop_stack(); // Value
                        self.pop_stack(); // Instance
                        self.push_stack(value);
                    }
                    Value::Object(obj) => {
                        let frame = self.current_frame();
                        let name = frame.read_constant(byte).as_string().unwrap();
                        obj.borrow_mut(self.mc).fields.insert(name, value);

                        let value = self.pop_stack(); // Value
                        self.pop_stack(); // Object
                        self.push_stack(value);
                    }
                    _ => return Err(self.runtime_error("Only instances have fields.".into())),
                }
            }
            OpCode::Method(byte) => {
                let name = frame.read_constant(byte).as_string().unwrap();
                self.define_method(name);
            }
            OpCode::Invoke {
                method_constant,
                positional_count,
                keyword_count,
            } => {
                let method_name = frame.read_constant(method_constant).as_string().unwrap();
                self.invoke(method_name, positional_count, keyword_count)?;
            }
            OpCode::Inherit => {
                if let Value::Class(superclass) = self.peek(1) {
                    let subclass = self.peek(0).as_class()?;
                    subclass
                        .borrow_mut(self.mc)
                        .methods
                        .extend(&superclass.borrow().methods);
                    self.pop_stack(); // Subclass
                } else {
                    return Err(self.runtime_error("Superclass must be a class.".into()));
                }
            }
            OpCode::GetSuper(byte) => {
                let name = frame.read_constant(byte).as_string().unwrap();
                let superclass = self.pop_stack().as_class()?;
                self.bind_method(superclass, name)?
            }
            OpCode::SuperInvoke {
                method_constant,
                positional_count,
                keyword_count,
            } => {
                let method_name = frame.read_constant(method_constant).as_string().unwrap();
                let superclass = self.pop_stack().as_class()?;
                self.invoke_from_class(superclass, method_name, positional_count, keyword_count)?;
            }
            OpCode::MakeObject(count) => {
                let mut object = Object::default();
                let count = count as usize;

                // Stack has pairs of [key1, value1, key2, value2, ...]
                // Process from last to first pair
                for _ in (0..count).rev() {
                    let value = self.pop_stack();
                    let key = self
                        .pop_stack()
                        .as_string()
                        .map_err(|_| self.runtime_error("Object key must be a string.".into()))?;

                    object.fields.insert(key, value);
                }

                let object = Gc::new(self.mc, RefLock::new(object));
                self.push_stack(Value::Object(object));
            }
            OpCode::MakeArray(count) => {
                let count = count as usize;
                let elements: Vec<Value<'gc>> = self.pop_stack_n(count);
                let array = Value::Array(Gc::new(self.mc, RefLock::new(elements)));
                self.push_stack(array);
            }
            OpCode::GetIndex => {
                // Stack: [object] [key]
                let key = self.pop_stack();
                let target = self.pop_stack();

                match target {
                    Value::Object(obj) => {
                        // Convert key to string
                        let key = key.as_string().map_err(|_| {
                            self.runtime_error("Index key must be a string.".into())
                        })?;

                        // Get value from object's fields, default is nil if not key found.
                        let value = obj.borrow().fields.get(&key).copied().unwrap_or_default();
                        self.push_stack(value);
                    }
                    Value::Array(array) => {
                        let index = key.as_number().map_err(|_| {
                            self.runtime_error("Array index must be a number.".into())
                        })?;
                        let array = array.borrow();
                        let value = array.get(index as usize).copied().unwrap_or(Value::Nil);
                        self.push_stack(value);
                    }
                    Value::Instance(_) => {
                        return Err(self.runtime_error(
                            "Use dot notation for accessing instance properties.".into(),
                        ));
                    }
                    _ => {
                        return Err(
                            self.runtime_error("Only object and array support indexing.".into())
                        );
                    }
                }
            }
            OpCode::SetIndex => {
                // Stack: [object] [key] [value]
                let value = self.pop_stack();
                let index = self.pop_stack();
                let target = self.pop_stack();
                match target {
                    Value::Object(obj) => {
                        // Pop remaining operands now that we know they're valid
                        // Set the field
                        let key = index.as_string().unwrap();
                        obj.borrow_mut(self.mc).fields.insert(key, value);
                        // Push value back for assignment expressions
                        self.push_stack(value);
                    }
                    Value::Array(array) => {
                        let index = index.as_number().unwrap();
                        let mut array = array.borrow_mut(self.mc);
                        let index = index as usize;

                        // Grow array if needed
                        if index >= array.len() {
                            array.resize(index + 1, Value::Nil);
                        }
                        array[index] = value;
                        self.push_stack(value);
                    }
                    Value::Instance(_) => {
                        return Err(self.runtime_error(
                            "Use dot notation for accessing instance properties.".into(),
                        ));
                    }
                    _ => {
                        return Err(
                            self.runtime_error("Only object and array support indexing.".into())
                        );
                    }
                }
            }
            OpCode::In => {
                let target = self.pop_stack();
                let value = self.pop_stack();

                let result = match target {
                    Value::Array(array) => {
                        let array = array.borrow();
                        array.contains(&value)
                    }
                    Value::Object(obj) => {
                        let key = value.as_string().map_err(|_| {
                            self.runtime_error(
                                "Object key must be a string in 'in' operator.".into(),
                            )
                        })?;
                        obj.borrow().fields.contains_key(&key)
                    }
                    _ => {
                        return Err(self.runtime_error(
                            "Right operand of 'in' operator must be array or object.".into(),
                        ));
                    }
                };

                self.push_stack(Value::Boolean(result));
            }
            OpCode::Prompt => {
                let message = self.pop_stack().as_string().unwrap().to_string();
                let result = Value::from(self.intern(ai::prompt(message).as_bytes()));
                self.push_stack(result);
            }
            OpCode::Agent(name) => {
                let agent = frame.read_constant(name);
                self.push_stack(agent);
            }
            OpCode::ImportModule(module_name_idx) => {
                let module_name = frame.read_constant(module_name_idx).as_string()?;
                self.import_module(module_name)?;
            }
            OpCode::GetModuleVar {
                module_name_constant,
                var_name_constant,
            } => {
                let module_name = frame.read_constant(module_name_constant).as_string()?;
                let var_name = frame.read_constant(var_name_constant).as_string()?;

                if let Some(value) = self.module_manager.get_export(module_name, var_name) {
                    self.push_stack(value);
                } else {
                    return Err(self.runtime_error(
                        format!(
                            "Undefined variable '{}' in module '{}'",
                            var_name, module_name
                        )
                        .into(),
                    ));
                }
            }
        }
        Ok(None)
    }

    pub fn define_global(
        &mut self,
        name: InternedString<'gc>,
        value: Value<'gc>,
        visibility: Visibility,
    ) {
        // Always define in current globals scope
        self.globals.insert(name, value);

        // If public and in a module context, also add to module exports
        if visibility == Visibility::Public {
            if let Some(current_module) = self.current_module {
                if let Some(module) = self.module_manager.modules.get_mut(&current_module) {
                    #[cfg(feature = "debug")]
                    println!("Exporting {} from module {}", name, module.name());

                    module.add_export(name, value);
                }
            }
        }
    }

    pub fn eval_function(
        &mut self,
        chunk_id: ChunkId,
        params: &[Value<'gc>],
    ) -> Result<ReturnValue, VmError> {
        // Remember the current frame count in order to exit the loop at the correct frame.
        let frame_count = self.frame_count;
        self.call_function(chunk_id, params)?;

        loop {
            if let Some(result) = self.dispatch_next(frame_count)? {
                // Popup the call function pushed to the stack top
                self.pop_stack();
                return Ok(result);
            }
        }
    }

    // Runs the VM for a period of time controlled by the `fuel` parameter.
    //
    // Returns `Ok(false)` if the method has exhausted its fuel, but there is more work to
    // do, and returns `Ok(true)` if no more progress can be made.
    pub(super) fn step(&mut self, fuel: &mut Fuel) -> Result<Option<ReturnValue>, VmError> {
        loop {
            if let Some(result) = self.dispatch_next(0)? {
                return Ok(Some(result));
            }
            const FUEL_PER_STEP: i32 = 1;
            fuel.consume(FUEL_PER_STEP);

            if !fuel.should_continue() {
                return Ok(None);
            }
        }
    }

    fn capture_upvalue(&mut self, slot: usize) -> GcRefLock<'gc, UpvalueObj<'gc>> {
        let mut prev_upvalue = None;
        let mut open_upvalue = self.open_upvalues;
        while open_upvalue.map(|u| u.borrow().location > slot) == Some(true) {
            if let Some(upvalue) = open_upvalue {
                prev_upvalue = Some(upvalue);
                open_upvalue = upvalue.borrow().next;
            }
        }
        if let Some(upvalue) = open_upvalue {
            if upvalue.borrow().location == slot {
                // We found an existing upvalue capturing the variable,
                // so we reuse that upvalue.
                return upvalue;
            }
        }

        // Do not use peek() to get value! the slot would be incorrect offset to peek.
        // let local = &self.stack[slot].clone();
        // create a new upvalue for our local slot and insert it into the list at the right location.
        let created_upvalue = Gc::new(
            self.mc,
            RefLock::new(UpvalueObj {
                location: slot,
                closed: None,
                next: open_upvalue,
            }),
        );
        if let Some(prev) = prev_upvalue {
            prev.borrow_mut(self.mc).next = Some(created_upvalue);
        } else {
            self.open_upvalues = Some(created_upvalue);
        }
        created_upvalue
    }

    fn close_upvalues(&mut self, last: usize) {
        loop {
            if self.open_upvalues.map(|u| u.borrow().location < last) == Some(true) {
                break;
            }

            if let Some(upvalue) = self.open_upvalues.take() {
                let mut upvalue = upvalue.borrow_mut(self.mc);
                let local = self.stack[upvalue.location];
                upvalue.closed = Some(local);
                // Dummy location after closed assigned
                // In C's version, the location is a pointer to upvalue.closed
                // upvalue.location = 0;
                self.open_upvalues = upvalue.next;
            } else {
                break;
            }
        }
    }

    fn define_method(&mut self, name: InternedString<'gc>) {
        let class = self.peek(1).as_class().unwrap();
        class
            .borrow_mut(self.mc)
            .methods
            .insert(name, *self.peek(0));
        // pop the closure since we’re done with it.
        self.pop_stack();
    }

    pub fn define_builtins(&mut self) {
        self.define_native_function("clock", builtins::clock);
    }

    fn define_native_function(&mut self, name: &'static str, function: NativeFn<'gc>) {
        let s = self.intern_static(name);
        self.globals.insert(s, Value::NativeFunction(function));
    }

    fn bind_method(
        &mut self,
        class: GcRefLock<'gc, Class<'gc>>,
        name: InternedString<'gc>,
    ) -> Result<(), VmError> {
        if let Some(method) = class.borrow().methods.get(&name) {
            let bound = BoundMethod::new(*self.peek(0), (*method).as_closure()?);
            // pop the instance and replace the top of
            // the stack with the bound method.
            self.pop_stack();
            self.push_stack(Value::from(Gc::new(self.mc, bound)));
            Ok(())
        } else {
            Err(self.runtime_error(format!("Undefined property '{}'.", name).into()))
        }
    }

    fn call_value(
        &mut self,
        callee: Value<'gc>,
        args_count: u8,
        keyword_args_count: u8,
    ) -> Result<(), VmError> {
        let args_slot_count = (args_count + keyword_args_count * 2) as usize;
        match callee {
            Value::BoundMethod(bound) => {
                // inserts the receiver into the new CallFrame's slot zero.
                // normally, the receiver is 'this' or 'super' keyword.
                /*
                   Diagram for this: scone.topping("berries", "cream");

                                                   stackTop
                                                       |
                    <-- -1 --> <------ argCount ---->  |
                       0         1         2         3 v
                       |         |         |         |
                       v         v         v         v
                   +----------+-----------+-----------+---
                   | script   |fn topping()| "berries" | "cream"
                   +----------+-----------+-----------+---
                       ^                               ^
                       |                               |
                       +-------------------------------+
                           topping Callframe
                */
                self.stack[self.stack_top - args_slot_count - 1] = bound.receiver;
                self.call(bound.method, args_count, keyword_args_count)
            }
            Value::Class(class) => {
                let instance = Instance::new(class);
                self.stack[self.stack_top - args_slot_count - 1] =
                    Value::from(Gc::new(self.mc, RefLock::new(instance)));
                let init = self.intern_static("init");

                if let Some(initializer) = class.borrow().methods.get(&init) {
                    self.call(initializer.as_closure()?, args_count, keyword_args_count)
                } else if (args_count + keyword_args_count) != 0 {
                    Err(self.runtime_error(
                        format!(
                            "Expected 0 arguments but got {}.",
                            args_count + keyword_args_count
                        )
                        .into(),
                    ))
                } else {
                    Ok(())
                }
            }
            Value::Closure(closure) => self.call(closure, args_count, keyword_args_count),
            Value::NativeFunction(function) => {
                // Native functions don't support keyword args yet
                if keyword_args_count > 0 {
                    return Err(self.runtime_error(
                        "Native functions don't support keyword arguments.".into(),
                    ));
                }
                let result = function(self.mc, self.pop_stack_n(args_count as usize))?;
                self.stack_top -= 1; // Remove the function
                self.push_stack(result);
                Ok(())
            }
            _ => Err(self.runtime_error("Can only call functions and classes.".into())),
        }
    }

    fn invoke_from_class(
        &mut self,
        class: GcRefLock<'gc, Class<'gc>>,
        name: InternedString<'gc>,
        arg_count: u8,
        keyword_args_count: u8,
    ) -> Result<(), VmError> {
        if let Some(method) = class.borrow().methods.get(&name) {
            self.call(method.as_closure()?, arg_count, keyword_args_count)
        } else {
            Err(self.runtime_error(format!("Undefined property '{}'.", name).into()))
        }
    }

    fn invoke(
        &mut self,
        name: InternedString<'gc>,
        args_count: u8,
        keyword_args_count: u8,
    ) -> Result<(), VmError> {
        let args_slot_count = (args_count + keyword_args_count * 2) as usize;
        let receiver = *self.peek(args_slot_count);
        match receiver {
            Value::Instance(instance) => {
                if let Some(value) = instance.borrow().fields.get(&name) {
                    self.stack[self.stack_top - args_slot_count - 1] = *value;
                    self.call_value(*value, args_count, keyword_args_count)
                } else {
                    self.invoke_from_class(
                        instance.borrow().class,
                        name,
                        args_count,
                        keyword_args_count,
                    )
                }
            }
            Value::Module(module_name) => {
                // Handle module function invocation
                if let Some(value) = self.module_manager.get_export(module_name, name) {
                    // Replace the module value with the function value
                    self.stack[self.stack_top - args_slot_count - 1] = value;
                    // Now call the function
                    self.call_value(value, args_count, keyword_args_count)
                } else {
                    Err(self.runtime_error(
                        format!("Undefined function '{}' in module '{}'", name, module_name).into(),
                    ))
                }
            }
            Value::Agent(agent) => {
                if let Some(method) = agent.methods.get(&name) {
                    let args = self.check_args(method, args_count, keyword_args_count)?;
                    // Pop the arguments from the stack.
                    // The stack before call run_agent:
                    // [ <fn script> ][ agent Triage ][ debug ][ true ][ input ][ some message ]
                    // 0033    | OP_INVOKE        (0 args) 17 'run'
                    // The stack after called run_agent:
                    // [ <fn script> ][ agent Triage ]
                    self.stack_top -= (args_count + keyword_args_count * 2) as usize;
                    let result = ai::run_agent(self, agent, args);
                    self.push_stack(result);
                    Ok(())
                } else {
                    Err(self
                        .runtime_error(format!("Agent have no method called '{}'.", name).into()))
                }
            }
            _ => Err(self.runtime_error("Only instances or modules have methods.".into())),
        }
    }

    fn check_args(
        &mut self,
        function: &Gc<'gc, Function<'gc>>,
        args_count: u8,
        keyword_args_count: u8,
    ) -> Result<Vec<Value<'gc>>, VmError> {
        if args_count != function.arity && function.arity == function.max_arity {
            // No keyword arguments, simply compare the positional arguments number
            return Err(self.runtime_error(
                format!(
                    "Expected {} arguments but got {}.",
                    function.arity, args_count
                )
                .into(),
            ));
        }

        if self.frame_count == FRAME_MAX_SIZE {
            return Err(self.runtime_error("Stack overflow.".into()));
        }

        let max_arity = function.max_arity as usize;
        let mut final_args = vec![Value::Nil; max_arity];

        // Copy positional arguments
        let total_args = args_count as usize;
        let keyword_slots = keyword_args_count as usize * 2;

        if total_args > 0 {
            let arg_start = self.stack_top - total_args - keyword_slots;
            final_args[..total_args]
                .copy_from_slice(&self.stack[arg_start..(total_args + arg_start)]);
        }

        // Process keyword arguments
        if keyword_args_count > 0 {
            let kw_start = self.stack_top - keyword_slots;
            for i in 0..keyword_args_count as usize {
                let idx = kw_start + i * 2;
                let name = self.stack[idx].as_string().map_err(|_| {
                    self.runtime_error("Keyword argument name must be a string.".into())
                })?;
                let value = self.stack[idx + 1];

                if let Some(&(pos, _)) = function.params.get(&name) {
                    let pos = pos as usize;
                    if pos < total_args {
                        return Err(self.runtime_error(
                            format!("Keyword argument '{}' was already specified as positional argument.", name)
                            .into()
                        ));
                    }
                    final_args[pos] = value;
                } else {
                    return Err(
                        self.runtime_error(format!("Unknown keyword argument '{}'.", name).into())
                    );
                }
            }
        }

        // Fill in default values and check required parameters
        for (name, (pos, default)) in &function.params {
            let pos = *pos as usize;
            if final_args[pos].equals(&Value::Nil) {
                if pos < function.arity as usize && default.is_nil() {
                    return Err(
                        self.runtime_error(format!("Missing required argument '{}'.", name).into())
                    );
                }
                final_args[pos] = *default;
            }
        }

        Ok(final_args)
    }

    pub fn call(
        &mut self,
        closure: Gc<'gc, Closure<'gc>>,
        args_count: u8,
        keyword_args_count: u8,
    ) -> Result<(), VmError> {
        let function = &closure.function;

        let final_args = self.check_args(function, args_count, keyword_args_count)?;

        self.stack_top -= args_count as usize + keyword_args_count as usize * 2;
        let slot_start = self.stack_top - 1; // -1 for the function itself

        for arg in final_args {
            self.push_stack(arg);
        }

        // Create the call frame
        let call_frame = CallFrame {
            closure,
            ip: 0,
            slot_start,
        };

        self.frames.push(call_frame);
        self.frame_count += 1;

        Ok(())
    }
    #[inline(always)]
    pub fn push_stack(&mut self, value: Value<'gc>) {
        debug_assert!(self.stack_top < STACK_MAX_SIZE, "Stack overflow");
        unsafe {
            *self.stack.get_unchecked_mut(self.stack_top) = value;
        }
        self.stack_top += 1;
    }

    #[inline(always)]
    pub fn pop_stack(&mut self) -> Value<'gc> {
        // debug_assert!(self.stack_top > 0, "Stack underflow");
        if self.stack_top == 0 {
            return Value::Nil;
        }
        self.stack_top -= 1;
        unsafe { *self.stack.get_unchecked(self.stack_top) }
    }

    #[inline(always)]
    fn peek(&self, distance: usize) -> &Value<'gc> {
        debug_assert!(self.stack_top > distance, "Stack peek out of bounds");
        unsafe { self.stack.get_unchecked(self.stack_top - 1 - distance) }
    }

    fn pop_stack_n(&mut self, n: usize) -> Vec<Value<'gc>> {
        if n == 0 {
            return Vec::new();
        }

        // Ensure we don't pop more items than are on the stack
        let n = n.min(self.stack_top);

        let new_top = self.stack_top - n;
        let mut result = Vec::with_capacity(n);

        // Copy values from the stack to the result vector
        result.extend_from_slice(&self.stack[new_top..self.stack_top]);

        // Update the stack top
        self.stack_top = new_top;

        // No need to reverse as we're copying from bottom to top
        result
    }

    #[cfg(feature = "debug")]
    fn print_stack(&self) {
        print!("          ");
        for value in self.stack.iter().take(self.stack_top) {
            print!("[ ");
            print!("{value}");
            print!(" ]")
        }
        println!();
    }
}
