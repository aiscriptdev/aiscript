use std::collections::HashMap;
use std::fmt::Display;

use aiscript_directive::Validator;
use gc_arena::Collect;
use indexmap::IndexMap;

use crate::object::FunctionType;
use crate::{lexer::Token, ty::PrimitiveType};
use crate::{string::InternedString, Value};

mod pretty;

/// Use u16 to represent the chunk id
/// It is enough for a program to assign id for each function chunk.
pub type ChunkId = u16;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum Mutability {
    #[default]
    Mutable,
    Immutable,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Visibility {
    #[default]
    Private, // Default visibility
    Public, // Accessible from other modules
            // Could add more in future like:
            // Protected,  // Only accessible to child classes
            // Package,    // Only accessible within the same package/directory
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Literal<'gc> {
    Number(f64),
    String(InternedString<'gc>),
    Boolean(bool),
    #[default]
    Nil,
}

impl<'gc> Display for Literal<'gc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Number(n) => write!(f, "{n}"),
            Literal::String(s) => write!(f, "\"{s}\""),
            Literal::Boolean(b) => write!(f, "{b}"),
            Literal::Nil => write!(f, "nil"),
        }
    }
}

#[derive(Debug)]
pub struct EnumDecl<'gc> {
    pub name: Token<'gc>,
    pub variants: Vec<EnumVariant<'gc>>,
    pub methods: Vec<Stmt<'gc>>,
    pub visibility: Visibility,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct EnumVariant<'gc> {
    pub name: Token<'gc>,
    // Default is Literal::Nil
    pub value: Literal<'gc>,
}

#[derive(Debug)]
pub struct FunctionDecl<'gc> {
    pub name: Token<'gc>,
    pub mangled_name: String,
    pub doc: Option<Token<'gc>>,
    pub params: IndexMap<Token<'gc>, ParameterDecl<'gc>>,
    pub return_type: Option<Token<'gc>>,
    pub error_types: Vec<Token<'gc>>,
    pub body: Vec<Stmt<'gc>>,
    pub fn_type: FunctionType,
    pub visibility: Visibility,
    pub line: u32,
}

#[derive(Debug)]
pub struct VariableDecl<'gc> {
    pub name: Token<'gc>,
    pub initializer: Option<Expr<'gc>>,
    pub visibility: Visibility,
    pub line: u32,
}

pub struct ClassFieldDecl<'gc> {
    pub name: Token<'gc>,
    pub type_hint: Token<'gc>,
    pub default_value: Option<Expr<'gc>>,
    pub validators: Vec<Box<dyn Validator>>,
    pub line: u32,
}

#[derive(Debug)]
pub struct ClassDecl<'gc> {
    pub name: Token<'gc>,
    pub superclass: Option<Expr<'gc>>,
    // pub fields: Vec<ClassFieldDecl<'gc>>,
    pub methods: Vec<Stmt<'gc>>,
    pub visibility: Visibility,
    pub line: u32,
}

#[derive(Debug)]
pub struct AgentDecl<'gc> {
    pub name: Token<'gc>,
    pub mangled_name: String,
    pub fields: HashMap<&'gc str, Expr<'gc>>,
    pub tools: Vec<Stmt<'gc>>,
    pub visibility: Visibility,
    pub line: u32,
}

pub struct ParameterDecl<'gc> {
    pub name: Token<'gc>,
    pub type_hint: Option<Token<'gc>>,
    pub default_value: Option<Expr<'gc>>,
    pub validators: Vec<Box<dyn Validator>>,
}

impl<'gc> std::fmt::Debug for ParameterDecl<'gc> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parameter")
            .field("name", &self.name)
            .field("type_hint", &self.type_hint)
            .field("default_value", &self.default_value)
            .field("validators", &self.validators.len())
            .finish()
    }
}

impl<'gc> ParameterDecl<'gc> {
    pub fn new(name: Token<'gc>) -> Self {
        Self {
            name,
            type_hint: None,
            default_value: None,
            validators: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct ErrorHandler<'gc> {
    pub error_var: Token<'gc>,
    pub handler_body: Vec<Stmt<'gc>>,
    pub propagate: bool, // Whether to use ? operator
}

#[derive(Debug, Clone, Collect)]
#[collect(require_static)]
pub struct FnDef {
    pub chunk_id: ChunkId,
    pub doc: String,
    pub params: IndexMap<String, PrimitiveType>,
}

impl FnDef {
    pub fn new(
        chunk_id: ChunkId,
        doc: &Option<Token>,
        params: IndexMap<String, PrimitiveType>,
    ) -> Self {
        FnDef {
            chunk_id,
            doc: doc.map(|t| t.lexeme.to_owned()).unwrap_or_default(),
            params,
        }
    }
}

#[derive(Debug)]
pub enum ObjectProperty<'gc> {
    // Regular property with literal name
    Literal {
        key: Token<'gc>,
        value: Box<Expr<'gc>>,
    },
    // Computed property name
    Computed {
        key_expr: Box<Expr<'gc>>,
        value: Box<Expr<'gc>>,
    },
}

#[derive(Debug)]
pub struct MatchArm<'gc> {
    pub patterns: Vec<MatchPattern<'gc>>,
    pub body: Box<Expr<'gc>>,
    // Optional if guard
    pub guard: Option<Box<Expr<'gc>>>,
    pub line: u32,
}

#[derive(Debug)]
pub enum MatchPattern<'gc> {
    EnumVariant {
        enum_name: Token<'gc>,
        variant: Token<'gc>,
    },
    Literal {
        value: Literal<'gc>,
    },
    Variable {
        name: Token<'gc>,
    },
    Range {
        start: Option<Box<Expr<'gc>>>,
        end: Option<Box<Expr<'gc>>>,
        inclusive: bool,
    },
    Wildcard,
}

impl<'gc> MatchPattern<'gc> {
    pub fn is_variable_pattern(&self) -> bool {
        matches!(self, Self::Variable { .. })
    }
}

#[derive(Debug)]
pub enum Expr<'gc> {
    EnvLookup {
        expr: Box<Expr<'gc>>,
        line: u32,
    },
    Object {
        properties: Vec<ObjectProperty<'gc>>,
        line: u32,
    },
    EnumVariant {
        enum_name: Token<'gc>,
        variant: Token<'gc>,
        line: u32,
    },
    // syntax: [Enum::Variant] to get the value
    EvaluateVariant {
        expr: Box<Expr<'gc>>,
        line: u32,
    },
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
    Index {
        object: Box<Expr<'gc>>,
        key: Box<Expr<'gc>>,
        value: Option<Box<Expr<'gc>>>,
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
    Lambda {
        params: Vec<Token<'gc>>,
        body: Box<Expr<'gc>>,
        line: u32,
    },
    Block {
        statements: Vec<Stmt<'gc>>,
        line: u32,
    },
    Call {
        callee: Box<Expr<'gc>>,
        is_constructor: bool,
        arguments: Vec<Expr<'gc>>,
        keyword_args: HashMap<String, Expr<'gc>>,
        error_handler: Option<ErrorHandler<'gc>>,
        line: u32,
    },
    Invoke {
        object: Box<Expr<'gc>>,
        method: Token<'gc>,
        arguments: Vec<Expr<'gc>>,
        keyword_args: HashMap<String, Expr<'gc>>,
        error_handler: Option<ErrorHandler<'gc>>,
        line: u32,
    },
    Match {
        expr: Box<Expr<'gc>>,
        arms: Vec<MatchArm<'gc>>,
        line: u32,
    },
    InlineIf {
        condition: Box<Expr<'gc>>,
        then_branch: Box<Expr<'gc>>,
        else_branch: Box<Expr<'gc>>,
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
    Self_ {
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
        model: Option<Box<Expr<'gc>>>,
        line: u32,
    },
}

impl<'gc> Expr<'gc> {
    pub fn line(&self) -> u32 {
        match self {
            Self::EnvLookup { line, .. }
            | Self::Object { line, .. }
            | Self::EnumVariant { line, .. }
            | Self::EvaluateVariant { line, .. }
            | Self::Binary { line, .. }
            | Self::Grouping { line, .. }
            | Self::Array { line, .. }
            | Self::Literal { line, .. }
            | Self::Unary { line, .. }
            | Self::Variable { line, .. }
            | Self::Index { line, .. }
            | Self::Match { line, .. }
            | Self::InlineIf { line, .. }
            | Self::Assign { line, .. }
            | Self::And { line, .. }
            | Self::Or { line, .. }
            | Self::Lambda { line, .. }
            | Self::Block { line, .. }
            | Self::Call { line, .. }
            | Self::Invoke { line, .. }
            | Self::Get { line, .. }
            | Self::Set { line, .. }
            | Self::Self_ { line, .. }
            | Self::Super { line, .. }
            | Self::SuperInvoke { line, .. }
            | Self::Prompt { line, .. } => *line,
        }
    }
}

#[derive(Debug)]
pub enum Stmt<'gc> {
    Use {
        path: Token<'gc>,
        line: u32,
    },
    Enum(EnumDecl<'gc>),
    Expression {
        expression: Expr<'gc>,
        line: u32,
    },
    Let(VariableDecl<'gc>),
    Const {
        name: Token<'gc>,
        initializer: Expr<'gc>,
        visibility: Visibility,
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
    Function(FunctionDecl<'gc>),
    Raise {
        error: Expr<'gc>,
        line: u32,
    },
    Return {
        value: Option<Expr<'gc>>,
        line: u32,
    },
    // Block return just provides the block's value
    BlockReturn {
        value: Expr<'gc>,
        line: u32,
    },
    Class(ClassDecl<'gc>),
    Agent(AgentDecl<'gc>),
}

impl<'gc> Stmt<'gc> {
    pub fn line(&self) -> u32 {
        match self {
            Self::Use { line, .. }
            | Self::Enum(EnumDecl { line, .. })
            | Self::Expression { line, .. }
            | Self::Let(VariableDecl { line, .. })
            | Self::Const { line, .. }
            | Self::Break { line, .. }
            | Self::Continue { line, .. }
            | Self::Block { line, .. }
            | Self::If { line, .. }
            | Self::Loop { line, .. }
            | Self::Function(FunctionDecl { line, .. })
            | Self::Raise { line, .. }
            | Self::Return { line, .. }
            | Self::BlockReturn { line, .. }
            | Self::Class(ClassDecl { line, .. })
            | Self::Agent(AgentDecl { line, .. }) => *line,
        }
    }
}

// Implement PartialEq manually to handle float comparison
impl<'gc> PartialEq for Literal<'gc> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Literal::Number(a), Literal::Number(b)) => (a - b).abs() < f64::EPSILON,
            (Literal::String(a), Literal::String(b)) => a == b,
            (Literal::Boolean(a), Literal::Boolean(b)) => a == b,
            (Literal::Nil, Literal::Nil) => true,
            _ => false,
        }
    }
}

// Implement Eq after ensuring PartialEq handles float comparison correctly
impl<'gc> Eq for Literal<'gc> {}

// Implement Hash to match our Eq implementation
impl<'gc> std::hash::Hash for Literal<'gc> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Literal::Number(n) => {
                // Hash the bits of the float to be consistent with our Eq implementation
                n.to_bits().hash(state);
            }
            Literal::String(s) => s.hash(state),
            Literal::Boolean(b) => b.hash(state),
            Literal::Nil => 0.hash(state),
        }
    }
}

#[derive(Debug)]
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

impl<'gc> From<Literal<'gc>> for Value<'gc> {
    fn from(value: Literal<'gc>) -> Self {
        match value {
            Literal::Number(value) => Value::Number(value),
            Literal::String(value) => Value::String(value),
            Literal::Boolean(value) => Value::Boolean(value),
            Literal::Nil => Value::Nil,
        }
    }
}

impl<'gc> From<Box<Expr<'gc>>> for Expr<'gc> {
    fn from(value: Box<Expr<'gc>>) -> Self {
        *value
    }
}

impl<'gc> From<Box<Stmt<'gc>>> for Stmt<'gc> {
    fn from(value: Box<Stmt<'gc>>) -> Self {
        *value
    }
}
