use std::{
    fmt::Display,
    ops::{Index, IndexMut},
    sync::Once,
};

use gc_arena::Collect;

use crate::{
    ast::{ChunkId, Visibility},
    Value,
};

#[derive(Copy, Clone, Debug, Collect)]
#[collect(require_static)]
pub enum OpCode {
    Constant(u8),
    Return,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
    Negate,
    Nil,
    Bool(bool),
    Not,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Print,
    Pop,
    DefineGlobal {
        name_constant: u8,
        visibility: Visibility,
    },
    GetGlobal(u8),
    SetGlobal(u8),
    GetLocal(u8),
    SetLocal(u8),
    JumpIfFalse(u16),
    Jump(u16),
    Loop(u16),
    Call {
        positional_count: u8,
        keyword_count: u8,
    },
    Closure {
        chunk_id: ChunkId,
    },
    GetUpvalue(u8),
    SetUpvalue(u8),
    CloseUpvalue,
    Class(u8),
    SetProperty(u8),
    GetProperty(u8),
    Method(u8),
    Invoke {
        method_constant: u8,
        positional_count: u8,
        keyword_count: u8,
    },
    Inherit,
    GetSuper(u8),
    SuperInvoke {
        method_constant: u8,
        positional_count: u8,
        keyword_count: u8,
    },
    MakeObject(u8), //  number of key-value pairs in the object
    // Import a module, constant index contains module name
    ImportModule(u8),
    // Get variable from module (module name index, var name index)
    GetModuleVar {
        module_name_constant: u8,
        var_name_constant: u8,
    },
    // AI
    Prompt,
    Agent(u8), // constant index
}

impl OpCode {
    pub fn putch_jump(&mut self, jump: u16) {
        match self {
            OpCode::JumpIfFalse(j) => {
                *j = jump;
            }
            OpCode::Jump(j) => {
                *j = jump;
            }
            OpCode::Loop(j) => {
                *j = jump;
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug, Collect)]
#[collect[no_drop]]
pub struct Chunk<'gc> {
    #[collect(require_static)]
    pub code: Vec<OpCode>,
    constans: Vec<Value<'gc>>,
    #[collect(require_static)]
    lines: Vec<u32>,
}

impl<'gc> Default for Chunk<'gc> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'gc> Index<usize> for Chunk<'gc> {
    type Output = OpCode;
    fn index(&self, index: usize) -> &Self::Output {
        // &self.code[index]
        unsafe { self.code.get_unchecked(index) }
    }
}

impl<'gc> IndexMut<usize> for Chunk<'gc> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        // &mut self.code[index]
        unsafe { self.code.get_unchecked_mut(index) }
    }
}

impl<'gc> Chunk<'gc> {
    pub fn new() -> Self {
        Chunk {
            code: Vec::new(),
            constans: Vec::new(),
            lines: Vec::new(),
        }
    }

    pub fn shrink_to_fit(&mut self) {
        self.code.shrink_to_fit();
        self.constans.shrink_to_fit();
    }

    pub fn line(&self, offset: usize) -> u32 {
        self.lines[offset]
    }

    pub fn code_size(&self) -> usize {
        self.code.len()
    }

    pub fn write_code(&mut self, code: OpCode, line: u32) {
        self.write_byte(code, line);
    }

    pub fn write_byte(&mut self, byte: OpCode, line: u32) {
        self.code.push(byte);
        self.lines.push(line);
    }

    pub fn add_constant(&mut self, value: Value<'gc>) -> usize {
        self.constans.push(value);
        // return the index where the constant
        // was appended so that we can locate that same constant later
        self.constans.len() - 1
    }

    #[inline]
    pub fn read_constant(&self, byte: u8) -> Value<'gc> {
        // self.constans[byte as usize]
        unsafe { *self.constans.get_unchecked(byte as usize) }
    }

    pub fn disassemble(&self, name: impl Display) {
        println!("\n== {name} ==>");
        let mut offset = 0;
        while offset < self.code.len() {
            offset = self.disassemble_instruction(offset);
        }
        println!("<== {name} ==\n");
    }

    pub fn disassemble_instruction(&self, offset: usize) -> usize {
        static ONCE_TITLE: Once = Once::new();
        ONCE_TITLE.call_once(|| {
            println!("{:4} {:4} {:16} CIndex Constvalue", "IP", "Line", "OPCode",);
        });

        print!("{:04} ", offset);
        if offset > 0 && self.lines[offset] == self.lines[offset - 1] {
            print!("   | ");
        } else {
            print!("{:4} ", self.lines[offset]);
        }

        if let Some(code) = self.code.get(offset) {
            match *code {
                OpCode::Return => simple_instruction("RETURN"),
                OpCode::Constant(c) => self.constant_instruction("CONSTANT", c),
                OpCode::Add => simple_instruction("ADD"),
                OpCode::Subtract => simple_instruction("SUBTRACT"),
                OpCode::Multiply => simple_instruction("MULTIPLY"),
                OpCode::Divide => simple_instruction("DIVIDE"),
                OpCode::Modulo => simple_instruction("MODULO"),
                OpCode::Power => simple_instruction("POWER"),
                OpCode::Negate => simple_instruction("NEGATE"),
                OpCode::Nil => simple_instruction("NIL"),
                OpCode::Bool(b) => simple_instruction(if b { "TRUE" } else { "FALSE" }),
                OpCode::Not => simple_instruction("NOT"),
                OpCode::Equal => simple_instruction("EQUAL"),
                OpCode::NotEqual => simple_instruction("NOT_EQUAL"),
                OpCode::Greater => simple_instruction("GREATER"),
                OpCode::GreaterEqual => simple_instruction("GREATER_EQUAL"),
                OpCode::Less => simple_instruction("LESS"),
                OpCode::LessEqual => simple_instruction("LESS_EQUAL"),
                OpCode::Print => simple_instruction("PRINT"),
                OpCode::Pop => simple_instruction("POP"),
                OpCode::DefineGlobal { name_constant, .. } => {
                    self.constant_instruction("DEFINE_GLOBAL", name_constant)
                }
                OpCode::GetGlobal(c) => self.constant_instruction("GET_GLOBAL", c),
                OpCode::SetGlobal(c) => self.constant_instruction("SET_GLOBAL", c),
                OpCode::GetLocal(byte) => self.byte_instruction("GET_LOCAL", byte),
                OpCode::SetLocal(c) => self.byte_instruction("SET_LOCAL", c),
                OpCode::JumpIfFalse(jump) => {
                    self.jump_instruction("JUMP_IF_FALSE", 1, offset, jump)
                }
                OpCode::Jump(jump) => self.jump_instruction("JUMP", 1, offset, jump),
                OpCode::Loop(jump) => self.jump_instruction("LOOP", -1, offset, jump),
                OpCode::Call {
                    positional_count,
                    keyword_count,
                } => {
                    println!(
                        "{:-16} {:4} {:4}",
                        "OP_CALL", positional_count, keyword_count
                    );
                }
                OpCode::Closure { chunk_id } => {
                    // let mut offset = offset + 1;
                    // let constant = self.code[offset] as usize;
                    // offset += 1;
                    println!("{:-16} {:4}", "OP_CLOSURE", chunk_id);

                    // let function = self.constans[c as usize].as_closure().unwrap().function;
                    // function.upvalues.iter().for_each(|upvalue| {
                    //     let Upvalue { index, is_local } = *upvalue;
                    //     println!(
                    //         "{:04}    | {:-22} {:4} {}",
                    //         offset - 2,
                    //         "",
                    //         if is_local { "local" } else { "upvalue" },
                    //         index,
                    //     );
                    // });
                }
                OpCode::GetUpvalue(c) => self.byte_instruction("GET_UPVALUE", c),
                OpCode::SetUpvalue(c) => self.byte_instruction("SET_UPVALUE", c),
                OpCode::CloseUpvalue => simple_instruction("CLOSE_UPVALUE"),
                OpCode::Class(c) => self.constant_instruction("CLASS", c),
                OpCode::SetProperty(c) => self.constant_instruction("SET_PROPERTY", c),
                OpCode::GetProperty(c) => self.constant_instruction("GET_PROPERTY", c),
                OpCode::Method(c) => self.constant_instruction("METHOD", c),
                OpCode::Invoke {
                    method_constant,
                    positional_count,
                    ..
                } => self.invoke_instruction("INVOKE", method_constant, positional_count),
                OpCode::Inherit => simple_instruction("INHERIT"),
                OpCode::GetSuper(c) => self.constant_instruction("GET_SUPER", c),
                OpCode::SuperInvoke {
                    method_constant,
                    positional_count,
                    ..
                } => self.invoke_instruction("SUPER_INVOKE", method_constant, positional_count),
                OpCode::MakeObject(c) => self.constant_instruction("MAKE_OBJECT", c),
                OpCode::ImportModule(c) => self.constant_instruction("IMPORT_MODULE", c),
                OpCode::GetModuleVar {
                    module_name_constant,
                    var_name_constant,
                } => self.invoke_instruction(
                    "GET_MODULE_VAR",
                    module_name_constant,
                    var_name_constant,
                ),
                OpCode::Prompt => simple_instruction("PROMPT"),
                OpCode::Agent(c) => {
                    println!("{:-16} {:4} '{}'", "OP_AGENT", c, self.constans[c as usize]);
                }
            }
        } else {
            println!("Invalid opcode at offset: {offset}");
        }

        offset + 1
    }

    fn constant_instruction(&self, name: &str, constant: u8) {
        let name = format!("OP_{name}");
        println!(
            "{:-16} {:4} '{}'",
            name, constant, self.constans[constant as usize]
        );
    }

    fn byte_instruction(&self, name: &str, byte: u8) {
        let name = format!("OP_{name}");
        println!("{:-16} {:4}", name, byte);
    }

    fn jump_instruction(&self, name: &str, sign: i8, offset: usize, jump: u16) {
        let name = format!("OP_{name}");
        // let jump = u16::from_be_bytes([self.code[offset + 1], self.code[offset + 2]]);
        let jump = if sign < 0 {
            offset.saturating_sub(jump as usize)
        } else {
            offset.saturating_add(jump as usize)
        };

        println!("{:-16} {:4} -> {}", name, offset, jump);
    }

    fn invoke_instruction(&self, name: &str, constant: u8, arity: u8) {
        let name = format!("OP_{name}");
        println!(
            "{:-16} ({} args) {} '{}'",
            name, arity, constant, self.constans[constant as usize]
        );
    }
}

fn simple_instruction(name: &str) {
    println!("OP_{name}");
}
