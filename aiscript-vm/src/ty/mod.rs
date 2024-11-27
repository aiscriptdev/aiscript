mod r#enum;
mod resolver;

use crate::lexer::Token;
use gc_arena::Collect;
pub(crate) use r#enum::EnumVariantChecker;
pub(crate) use resolver::TypeResolver;

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
    // Class type reference (holds the type name token for error reporting)
    Class(Token<'gc>),
    Enum(Token<'gc>), // You can add more complex types here in the future
                      // Function(Vec<Type>, Box<Type>),  // For function types
                      // Array(Box<Type>),                // For array types
                      // Optional(Box<Type>),             // For optional types
}

impl<'gc> Type<'gc> {
    /// Convert a token to a type, handling both builtin and custom types
    pub fn from_token(token: Token<'gc>) -> Type<'gc> {
        match token.lexeme {
            "int" => Type::Int,
            "str" => Type::Str,
            "bool" => Type::Bool,
            "float" => Type::Float,
            _ => Type::Class(token),
        }
    }

    /// Get a human-readable name for the type
    pub fn type_name(&self) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::Str => "str".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Float => "float".to_string(),
            Type::Enum(token) => token.lexeme.to_string(),
            Type::Class(token) => token.lexeme.to_string(),
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
