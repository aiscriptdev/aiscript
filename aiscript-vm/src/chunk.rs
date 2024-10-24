use std::{
    fmt::Display,
    ops::{Index, IndexMut},
    sync::Once,
};

use gc_arena::Collect;

use crate::{object::Upvalue, Value};

#[derive(Copy, Clone, Debug, Collect)]
#[collect(no_drop)]
pub enum OpCode {
    Constant(u8),
    Return,
    Add,
    Subtract,
    Multiply,
    Divide,
    Negate,
    Nil,
    True,
    False,
    Not,
    Equal,
    Greater,
    Less,
    Print,
    Pop,
    DefineGlobal(u8),
    GetGlobal(u8),
    SetGlobal(u8),
    GetLocal(u8),
    SetLocal(u8),
    JumpIfFalse(u16),
    Jump(u16),
    Loop(u16),
    Call(u8),
    Closure(u8),
    GetUpvalue(u8),
    SetUpvalue(u8),
    CloseUpvalue,
    Class(u8),
    SetProperty(u8),
    GetProperty(u8),
    Method(u8),
    Invoke(u8, u8),
    Inherit,
    GetSuper(u8),
    SuperInvoke(u8, u8),
    Unknown,
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
    code: Vec<OpCode>,
    constans: Vec<Value<'gc>>,
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
                OpCode::Negate => simple_instruction("NEGATE"),
                OpCode::Nil => simple_instruction("NIL"),
                OpCode::True => simple_instruction("TRUE"),
                OpCode::False => simple_instruction("FALSE"),
                OpCode::Not => simple_instruction("NOT"),
                OpCode::Equal => simple_instruction("EQUAL"),
                OpCode::Greater => simple_instruction("GREATER"),
                OpCode::Less => simple_instruction("LESS"),
                OpCode::Print => simple_instruction("PRINT"),
                OpCode::Pop => simple_instruction("POP"),
                OpCode::DefineGlobal(c) => self.constant_instruction("DEFINE_GLOBAL", c),
                OpCode::GetGlobal(c) => self.constant_instruction("GET_GLOBAL", c),
                OpCode::SetGlobal(c) => self.constant_instruction("SET_GLOBAL", c),
                OpCode::GetLocal(byte) => self.byte_instruction("GET_LOCAL", byte),
                OpCode::SetLocal(c) => self.byte_instruction("SET_LOCAL", c),
                OpCode::JumpIfFalse(jump) => {
                    self.jump_instruction("JUMP_IF_FALSE", 1, offset, jump)
                }
                OpCode::Jump(jump) => self.jump_instruction("JUMP", 1, offset, jump),
                OpCode::Loop(jump) => self.jump_instruction("LOOP", -1, offset, jump),
                OpCode::Call(arity) => self.byte_instruction("CALL", arity),
                OpCode::Closure(c) => {
                    // let mut offset = offset + 1;
                    // let constant = self.code[offset] as usize;
                    // offset += 1;
                    println!(
                        "{:-16} {:4} '{}'",
                        "OP_CLOSURE", c, self.constans[c as usize]
                    );

                    let function = self.constans[c as usize].as_function().unwrap();
                    function.upvalues.iter().for_each(|upvalue| {
                        let Upvalue { index, is_local } = *upvalue;
                        println!(
                            "{:04}    | {:-22} {:4} {}",
                            offset - 2,
                            "",
                            if is_local { "local" } else { "upvalue" },
                            index,
                        );
                    });
                }
                OpCode::GetUpvalue(c) => self.byte_instruction("GET_UPVALUE", c),
                OpCode::SetUpvalue(c) => self.byte_instruction("SET_UPVALUE", c),
                OpCode::CloseUpvalue => simple_instruction("CLOSE_UPVALUE"),
                OpCode::Class(c) => self.constant_instruction("CLASS", c),
                OpCode::SetProperty(c) => self.constant_instruction("SET_PROPERTY", c),
                OpCode::GetProperty(c) => self.constant_instruction("GET_PROPERTY", c),
                OpCode::Method(c) => self.constant_instruction("METHOD", c),
                OpCode::Invoke(name, arity) => self.invoke_instruction("INVOKE", name, arity),
                OpCode::Inherit => simple_instruction("INHERIT"),
                OpCode::GetSuper(c) => self.constant_instruction("GET_SUPER", c),
                OpCode::SuperInvoke(name, arity) => {
                    self.invoke_instruction("SUPER_INVOKE", name, arity)
                }
                OpCode::Unknown => {}
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
