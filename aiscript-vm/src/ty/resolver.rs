use std::collections::HashMap;

use super::Type;

pub struct TypeResolver<'gc> {
    // Keep track of defined types (classes) in the current scope
    defined_types: HashMap<&'gc str, Type<'gc>>,
}

impl<'gc> TypeResolver<'gc> {
    pub fn new() -> Self {
        let mut resolver = Self {
            defined_types: HashMap::new(),
        };

        // Register built-in types
        // In practice, you might want to use string interning for these names
        resolver.defined_types.insert("int", Type::Int);
        resolver.defined_types.insert("str", Type::Str);
        resolver.defined_types.insert("bool", Type::Bool);
        resolver.defined_types.insert("float", Type::Float);

        resolver
    }

    /// Register a new custom type (called when processing class definitions)
    pub fn register_type(&mut self, name: &'gc str, typ: Type<'gc>) {
        self.defined_types.insert(name, typ);
    }

    /// Resolve a type reference, returning None if the type is not defined
    pub fn resolve_type(&self, typ: Type<'gc>) -> Option<Type<'gc>> {
        match typ {
            Type::Class(token) => self.defined_types.get(token.lexeme).copied(),
            _ => Some(typ), // Built-in types are always resolved
        }
    }

    /// Validate that a type exists (used during parsing/semantic analysis)
    pub fn validate_type(&self, typ: Type<'gc>) -> Result<(), String> {
        match self.resolve_type(typ) {
            Some(_) => Ok(()),
            None => match typ {
                Type::Class(token) => Err(format!(
                    "Undefined type '{}' at line {}",
                    token.lexeme, token.line
                )),
                _ => Ok(()),
            },
        }
    }
}
