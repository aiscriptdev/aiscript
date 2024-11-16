use std::collections::HashMap;

use gc_arena::Collect;
use indexmap::IndexMap;

use crate::{lexer::Token, ty::PrimitiveType};
use crate::{string::InternedString, Value};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Visibility {
    #[default]
    Private, // Default visibility
    Public, // Accessible from other modules
            // Could add more in future like:
            // Protected,  // Only accessible to child classes
            // Package,    // Only accessible within the same package/directory
}

#[derive(Debug, Clone)]
pub struct FunctionDecl<'gc> {
    pub name: Token<'gc>,
    pub mangled_name: String,
    pub doc: Option<Token<'gc>>,
    pub params: IndexMap<Token<'gc>, Parameter<'gc>>,
    pub return_type: Option<Token<'gc>>,
    pub body: Vec<Stmt<'gc>>,
    pub is_ai: bool,
    pub visibility: Visibility,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct VariableDecl<'gc> {
    pub name: Token<'gc>,
    pub initializer: Option<Expr<'gc>>,
    pub visibility: Visibility,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct ClassDecl<'gc> {
    pub name: Token<'gc>,
    pub superclass: Option<Expr<'gc>>,
    pub methods: Vec<Stmt<'gc>>,
    pub visibility: Visibility,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct AgentDecl<'gc> {
    pub name: Token<'gc>,
    pub mangled_name: String,
    pub fields: HashMap<&'gc str, Expr<'gc>>,
    pub tools: Vec<Stmt<'gc>>,
    pub visibility: Visibility,
    pub line: u32,
}

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
    Use {
        path: Token<'gc>,
        line: u32,
    },
    Expression {
        expression: Expr<'gc>,
        line: u32,
    },
    Print {
        expression: Expr<'gc>,
        line: u32,
    },
    Let(VariableDecl<'gc>),
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
    Function(FunctionDecl<'gc>),
    Return {
        value: Option<Expr<'gc>>,
        line: u32,
    },
    Class(ClassDecl<'gc>),
    Agent(AgentDecl<'gc>),
}

impl<'gc> Stmt<'gc> {
    pub fn line(&self) -> u32 {
        match self {
            Self::Use { line, .. }
            | Self::Expression { line, .. }
            | Self::Print { line, .. }
            | Self::Let(VariableDecl { line, .. })
            | Self::Break { line, .. }
            | Self::Continue { line, .. }
            | Self::Block { line, .. }
            | Self::If { line, .. }
            | Self::Loop { line, .. }
            | Self::Function(FunctionDecl { line, .. })
            | Self::Return { line, .. }
            | Self::Class(ClassDecl { line, .. })
            | Self::Agent(AgentDecl { line, .. }) => *line,
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
