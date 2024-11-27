use std::collections::HashMap;

use gc_arena::GcRefLock;

use crate::object::Enum;

use super::Type;

#[derive(Default)]
pub(crate) struct TypeResolver<'gc> {
    // Keep track of defined types (classes) in the current scope
    defined_types: HashMap<&'gc str, Type<'gc>>,
    // Keep track user defiend enums, help to allow
    // declare enum variant as default function arguments
    defined_enums: HashMap<&'gc str, GcRefLock<'gc, Enum<'gc>>>,
}

impl<'gc> TypeResolver<'gc> {
    pub fn new() -> Self {
        let mut resolver = Self {
            defined_types: HashMap::new(),
            defined_enums: HashMap::new(),
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

    pub fn register_enum(&mut self, name: &'gc str, enum_: GcRefLock<'gc, Enum<'gc>>) {
        self.defined_enums.insert(name, enum_);
    }

    pub fn get_enum(&mut self, name: &str) -> Option<GcRefLock<'gc, Enum<'gc>>> {
        self.defined_enums.get(name).copied()
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
                Type::Custom(token) => Err(format!("Undefined type '{}'", token.lexeme)),
                _ => Ok(()),
            },
        }
    }
}
