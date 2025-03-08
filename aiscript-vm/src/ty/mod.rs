use aiscript_arena::Collect;

mod r#enum;
mod error;
mod resolver;

use crate::lexer::Token;
pub(crate) use r#enum::EnumVariantChecker;
pub(crate) use error::FunctionErrorResolver;
pub(crate) use resolver::{ClassField, TypeResolver, ValidationError};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Collect)]
#[collect(require_static)]
pub enum PrimitiveType {
    Int,
    Str,
    Bool,
    Float,
    Enum,
    NonPrimitive,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Type<'gc> {
    Int,
    Str,
    Bool,
    Float,
    // User defined type, including: class, enum
    Custom(Token<'gc>),
}

impl<'gc> Type<'gc> {
    /// Convert a token to a type, handling both builtin and custom types
    pub fn from_token(token: Token<'gc>) -> Type<'gc> {
        match token.lexeme {
            "int" => Type::Int,
            "str" => Type::Str,
            "bool" => Type::Bool,
            "float" => Type::Float,
            _ => Type::Custom(token),
        }
    }

    /// Get a human-readable name for the type
    #[allow(unused)]
    pub fn type_name(&self) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::Str => "str".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Float => "float".to_string(),
            Type::Custom(token) => token.lexeme.to_string(),
        }
    }
}

impl<'gc> From<Token<'gc>> for PrimitiveType {
    fn from(token: Token<'gc>) -> Self {
        match token.lexeme {
            "int" => PrimitiveType::Int,
            "str" => PrimitiveType::Str,
            "bool" => PrimitiveType::Bool,
            "float" => PrimitiveType::Float,
            _ => PrimitiveType::NonPrimitive,
        }
    }
}
