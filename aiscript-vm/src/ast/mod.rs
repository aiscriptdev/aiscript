use std::collections::HashMap;

use gc_arena::Collect;
use indexmap::IndexMap;

use crate::{lexer::Token, ty::PrimitiveType};
use crate::{string::InternedString, Value};

#[derive(Debug, Clone)]
pub struct Parameter<'gc> {
    pub name: Token<'gc>,
    pub type_hint: Option<Token<'gc>>,
    pub default_value: Option<Expr<'gc>>,
}

impl<'gc> Parameter<'gc> {
    pub fn new(name: Token<'gc>) -> Self {
        Self {
            name,
            type_hint: None,
            default_value: None,
        }
    }

    pub fn with_type(mut self, type_hint: Token<'gc>) -> Self {
        self.type_hint = Some(type_hint);
        self
    }

    pub fn with_default(mut self, default_value: Expr<'gc>) -> Self {
        self.default_value = Some(default_value);
        self
    }
}

#[derive(Debug, Clone, Collect)]
#[collect(require_static)]
pub struct FnDef {
    pub chunk_id: usize,
    pub doc: String,
    pub params: IndexMap<String, PrimitiveType>,
}

impl FnDef {
    pub fn new<'gc>(
        chunk_id: usize,
        doc: &Option<Token<'gc>>,
        params: &IndexMap<Token<'gc>, Parameter<'gc>>,
    ) -> Self {
        FnDef {
            chunk_id,
            doc: doc.map(|t| t.lexeme.to_owned()).unwrap_or_default(),
            params: params
                .iter()
                .map(|(name, param)| {
                    (
                        name.lexeme.to_owned(),
                        PrimitiveType::from(param.type_hint.unwrap_or_default()),
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr<'gc> {
    Binary {
        left: Box<Expr<'gc>>,
        operator: Token<'gc>,
        right: Box<Expr<'gc>>,
        line: u32,
    },
    Grouping {
        expression: Box<Expr<'gc>>,
        line: u32,
    },
    Array {
        elements: Vec<Expr<'gc>>,
        line: u32,
    },
    Literal {
        value: Literal<'gc>,
        line: u32,
    },
    Unary {
        operator: Token<'gc>,
        right: Box<Expr<'gc>>,
        line: u32,
    },
    Variable {
        name: Token<'gc>,
        line: u32,
    },
    Assign {
        name: Token<'gc>,
        value: Box<Expr<'gc>>,
        line: u32,
    },
    And {
        left: Box<Expr<'gc>>,
        right: Box<Expr<'gc>>,
        line: u32,
    },
    Or {
        left: Box<Expr<'gc>>,
        right: Box<Expr<'gc>>,
        line: u32,
    },
    Call {
        callee: Box<Expr<'gc>>,
        arguments: Vec<Expr<'gc>>,
        keyword_args: HashMap<String, Expr<'gc>>,
        line: u32,
    },
    Invoke {
        object: Box<Expr<'gc>>,
        method: Token<'gc>,
        arguments: Vec<Expr<'gc>>,
        keyword_args: HashMap<String, Expr<'gc>>,
        line: u32,
    },
    Get {
        object: Box<Expr<'gc>>,
        name: Token<'gc>,
        line: u32,
    },
    Set {
        object: Box<Expr<'gc>>,
        name: Token<'gc>,
        value: Box<Expr<'gc>>,
        line: u32,
    },
    This {
        line: u32,
    },
    Super {
        method: Token<'gc>,
        line: u32,
    },
    SuperInvoke {
        method: Token<'gc>,
        arguments: Vec<Expr<'gc>>,
        keyword_args: HashMap<String, Expr<'gc>>,
        line: u32,
    },
    Prompt {
        expression: Box<Expr<'gc>>,
        line: u32,
    },
}

impl<'gc> Expr<'gc> {
    pub fn line(&self) -> u32 {
        match self {
            Self::Binary { line, .. }
            | Self::Grouping { line, .. }
            | Self::Array { line, .. }
            | Self::Literal { line, .. }
            | Self::Unary { line, .. }
            | Self::Variable { line, .. }
            | Self::Assign { line, .. }
            | Self::And { line, .. }
            | Self::Or { line, .. }
            | Self::Call { line, .. }
            | Self::Invoke { line, .. }
            | Self::Get { line, .. }
            | Self::Set { line, .. }
            | Self::This { line, .. }
            | Self::Super { line, .. }
            | Self::SuperInvoke { line, .. }
            | Self::Prompt { line, .. } => *line,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Stmt<'gc> {
    Expression {
        expression: Expr<'gc>,
        line: u32,
    },
    Print {
        expression: Expr<'gc>,
        line: u32,
    },
    Let {
        name: Token<'gc>,
        initializer: Option<Expr<'gc>>,
        line: u32,
    },
    Block {
        statements: Vec<Stmt<'gc>>,
        line: u32,
    },
    Break {
        line: u32,
    },
    Continue {
        line: u32,
    },
    If {
        condition: Expr<'gc>,
        then_branch: Box<Stmt<'gc>>,
        else_branch: Option<Box<Stmt<'gc>>>,
        line: u32,
    },
    Loop {
        initializer: Option<Box<Stmt<'gc>>>,
        condition: Expr<'gc>,
        increment: Option<Expr<'gc>>,
        body: Box<Stmt<'gc>>,
        line: u32,
    },
    Function {
        name: Token<'gc>,
        mangled_name: String,
        doc: Option<Token<'gc>>,
        params: IndexMap<Token<'gc>, Parameter<'gc>>,
        return_type: Option<Token<'gc>>,
        body: Vec<Stmt<'gc>>,
        is_ai: bool,
        line: u32,
    },
    Return {
        value: Option<Expr<'gc>>,
        line: u32,
    },
    Class {
        name: Token<'gc>,
        superclass: Option<Expr<'gc>>,
        methods: Vec<Stmt<'gc>>,
        line: u32,
    },
    Agent {
        name: Token<'gc>,
        mangled_name: String,
        fields: HashMap<&'gc str, Expr<'gc>>,
        tools: Vec<Stmt<'gc>>,
        line: u32,
    },
}

impl<'gc> Stmt<'gc> {
    pub fn line(&self) -> u32 {
        match self {
            Self::Expression { line, .. }
            | Self::Print { line, .. }
            | Self::Let { line, .. }
            | Self::Break { line, .. }
            | Self::Continue { line, .. }
            | Self::Block { line, .. }
            | Self::If { line, .. }
            | Self::Loop { line, .. }
            | Self::Function { line, .. }
            | Self::Return { line, .. }
            | Self::Class { line, .. }
            | Self::Agent { line, .. } => *line,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Literal<'gc> {
    Number(f64),
    String(InternedString<'gc>),
    Boolean(bool),
    Nil,
}

#[derive(Debug, Clone)]
pub struct Program<'gc> {
    pub statements: Vec<Stmt<'gc>>,
}

impl<'gc> Program<'gc> {
    pub fn new() -> Self {
        Self {
            statements: Vec::new(),
        }
    }
}

impl<'gc> From<&Literal<'gc>> for Value<'gc> {
    fn from(value: &Literal<'gc>) -> Self {
        match value {
            Literal::Number(value) => Value::Number(*value),
            Literal::String(value) => Value::String(*value),
            Literal::Boolean(value) => Value::Boolean(*value),
            Literal::Nil => Value::Nil,
        }
    }
}
