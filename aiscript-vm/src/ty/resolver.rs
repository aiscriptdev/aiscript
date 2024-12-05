use std::collections::HashMap;

use crate::lexer::Token;

use super::Type;

#[derive(Default, Debug)]
pub(crate) struct TypeResolver<'gc> {
    // Keep track of defined types (classes) in the current scope
    defined_types: HashMap<&'gc str, Type<'gc>>,
    // Track type tokens that need validation
    pending_validations: Vec<Token<'gc>>,
}

impl<'gc> TypeResolver<'gc> {
    pub fn new() -> Self {
        let mut resolver = Self {
            defined_types: HashMap::new(),
            pending_validations: Vec::new(),
        };

        // Register built-in types
        // In practice, you might want to use string interning for these names
        resolver.defined_types.insert("int", Type::Int);
        resolver.defined_types.insert("str", Type::Str);
        resolver.defined_types.insert("bool", Type::Bool);
        resolver.defined_types.insert("float", Type::Float);

        resolver
    }

    pub fn add_type_usage(&mut self, token: Token<'gc>) {
        self.pending_validations.push(token);
    }

    pub fn validate_all_types<F>(&self, mut f: F)
    where
        F: FnMut(Token<'gc>, String),
    {
        for token in &self.pending_validations {
            let ty = Type::from_token(*token);
            if let Err(err) = self.validate_type(ty) {
                f(*token, err);
            }
        }
    }

    /// Register a new custom type
    pub fn register_type(&mut self, name: &'gc str, typ: Type<'gc>) {
        self.defined_types.insert(name, typ);
    }

    /// Resolve a type reference, returning None if the type is not defined
    fn resolve_type(&self, typ: Type<'gc>) -> Option<Type<'gc>> {
        match typ {
            Type::Custom(token) => self.defined_types.get(token.lexeme).copied(),
            _ => Some(typ), // Built-in types are always resolved
        }
    }

    /// Validate that a type exists (used during parsing/semantic analysis)
    pub fn validate_type(&self, typ: Type<'gc>) -> Result<(), String> {
        match self.resolve_type(typ) {
            Some(_) => Ok(()),
            None => match typ {
                Type::Custom(token) => Err(format!("Undefined type '{}'.", token.lexeme)),
                _ => Ok(()),
            },
        }
    }
}
