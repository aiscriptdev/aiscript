use std::{
    ops::{Index, IndexMut},
    sync::Once,
};

use gc_arena::Collect;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::Value;

#[derive(Copy, Clone, Debug, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum OpCode {
    Constant,
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
    DefineGlobal,
    GetGlobal,
    SetGlobal,
    GetLocal,
    SetLocal,
    JumpIfFalse,
    Jump,
    Loop,
    Call,
    Closure,
    GetUpvalue,
    SetUpvalue,
    CloseUpvalue,
    Class,
    SetProperty,
    GetProperty,
    Method,
    Invoke,
    Inherit,
    GetSuper,
    SuperInvoke,
    Unknown,
}

#[derive(Clone, Debug, Collect)]
#[collect[no_drop]]
pub struct Chunk<'gc> {
    code: Vec<u8>,
    constans: Vec<Value<'gc>>,
    lines: Vec<u32>,
}

impl<'gc> Default for Chunk<'gc> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'gc> Index<usize> for Chunk<'gc> {
    type Output = u8;
    fn index(&self, index: usize) -> &Self::Output {
        &self.code[index]
    }
}

impl<'gc> IndexMut<usize> for Chunk<'gc> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.code[index]
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

    pub fn write_byte<T: Into<u8>>(&mut self, byte: T, line: u32) {
        self.code.push(byte.into());
        self.lines.push(line);
    }

    pub fn add_constant(&mut self, value: Value<'gc>) -> usize {
        self.constans.push(value);
        // return the index where the constant
        // was appended so that we can locate that same constant later
        self.constans.len() - 1
    }

    pub fn read_constant(&self, byte: u8) -> Value<'gc> {
        self.constans[byte as usize]
    }

    pub fn disassemble(&self, name: &str) {
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
            match OpCode::try_from(*code).unwrap_or(OpCode::Unknown) {
                OpCode::Return => return simple_instruction("RETURN", offset),
                OpCode::Constant => return self.constant_instruction("CONSTANT", offset),
                OpCode::Add => return simple_instruction("ADD", offset),
                OpCode::Subtract => return simple_instruction("SUBTRACT", offset),
                OpCode::Multiply => return simple_instruction("MULTIPLY", offset),
                OpCode::Divide => return simple_instruction("DIVIDE", offset),
                OpCode::Negate => return simple_instruction("NEGATE", offset),
                OpCode::Nil => return simple_instruction("NIL", offset),
                OpCode::True => return simple_instruction("TRUE", offset),
                OpCode::False => return simple_instruction("FALSE", offset),
                OpCode::Not => return simple_instruction("NOT", offset),
                OpCode::Equal => return simple_instruction("EQUAL", offset),
                OpCode::Greater => return simple_instruction("GREATER", offset),
                OpCode::Less => return simple_instruction("LESS", offset),
                OpCode::Print => return simple_instruction("PRINT", offset),
                OpCode::Pop => return simple_instruction("POP", offset),
                OpCode::DefineGlobal => return self.constant_instruction("DEFINE_GLOBAL", offset),
                OpCode::GetGlobal => return self.constant_instruction("GET_GLOBAL", offset),
                OpCode::SetGlobal => return self.constant_instruction("SET_GLOBAL", offset),
                OpCode::GetLocal => return self.byte_instruction("GET_LOCAL", offset),
                OpCode::SetLocal => return self.byte_instruction("SET_LOCAL", offset),
                OpCode::JumpIfFalse => return self.jump_instruction("JUMP_IF_FALSE", 1, offset),
                OpCode::Jump => return self.jump_instruction("JUMP", 1, offset),
                OpCode::Loop => return self.jump_instruction("LOOP", -1, offset),
                OpCode::Call => return self.byte_instruction("CALL", offset),
                OpCode::Closure => {
                    let mut offset = offset + 1;
                    let constant = self.code[offset] as usize;
                    offset += 1;
                    println!(
                        "{:-16} {:4} '{}'",
                        "OP_CLOSURE", constant, self.constans[constant]
                    );

                    let function = self.constans[constant].as_function().unwrap();
                    (0..function.upvalue_count as usize).for_each(|_| {
                        let is_local = self.code[offset] == 1;
                        offset += 1;
                        let index = self.code[offset];
                        offset += 1;
                        println!(
                            "{:04}    | {:-22} {:4} {}",
                            offset - 2,
                            "",
                            if is_local { "local" } else { "upvalue" },
                            index,
                        );
                    });
                    return offset;
                }
                OpCode::GetUpvalue => return self.byte_instruction("GET_UPVALUE", offset),
                OpCode::SetUpvalue => return self.byte_instruction("SET_UPVALUE", offset),
                OpCode::CloseUpvalue => return simple_instruction("CLOSE_UPVALUE", offset),
                OpCode::Class => return self.constant_instruction("CLASS", offset),
                OpCode::SetProperty => return self.constant_instruction("SET_PROPERTY", offset),
                OpCode::GetProperty => return self.constant_instruction("GET_PROPERTY", offset),
                OpCode::Method => return self.constant_instruction("METHOD", offset),
                OpCode::Invoke => return self.invoke_instruction("INVOKE", offset),
                OpCode::Inherit => return simple_instruction("INHERIT", offset),
                OpCode::GetSuper => return self.constant_instruction("GET_SUPER", offset),
                OpCode::SuperInvoke => return self.invoke_instruction("SUPER_INVOKE", offset),
                OpCode::Unknown => {}
            }
        } else {
            println!("Invalid opcode at offset: {offset}");
        }

        offset + 1
    }

    fn constant_instruction(&self, name: &str, offset: usize) -> usize {
        let name = format!("OP_{name}");
        let constant = self.code[offset + 1];
        println!(
            "{:-16} {:4} '{}'",
            name, constant, self.constans[constant as usize]
        );
        offset + 2
    }

    fn byte_instruction(&self, name: &str, offset: usize) -> usize {
        let name = format!("OP_{name}");
        let slot = self.code[offset + 1];
        println!("{:-16} {:4}", name, slot);
        offset + 2
    }

    fn jump_instruction(&self, name: &str, sign: i8, offset: usize) -> usize {
        let name = format!("OP_{name}");
        let jump = u16::from_be_bytes([self.code[offset + 1], self.code[offset + 2]]);
        let jump = if sign < 0 {
            offset.saturating_add(3).saturating_sub(jump as usize)
        } else {
            offset.saturating_add(3).saturating_add(jump as usize)
        };

        println!("{:-16} {:4} -> {}", name, offset, jump);
        offset + 3
    }

    fn invoke_instruction(&self, name: &str, offset: usize) -> usize {
        let name = format!("OP_{name}");
        let constant = self.code[offset + 1];
        let arg_count = self.code[offset + 2];
        println!(
            "{:-16} ({} args) {} '{}'",
            name, arg_count, constant, self.constans[constant as usize]
        );
        offset + 3
    }
}

fn simple_instruction(name: &str, offset: usize) -> usize {
    println!("OP_{name}");
    offset + 1
}
