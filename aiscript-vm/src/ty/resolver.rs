use std::collections::{HashMap, HashSet};

use crate::{
    ast::{Expr, Literal, ObjectProperty},
    lexer::Token,
};

use super::Type;

#[derive(Debug)]
pub(crate) struct ClassField<'gc> {
    pub name: Token<'gc>,
    pub ty: Type<'gc>,
    pub required: bool,
}

#[derive(Debug)]
pub(crate) struct ClassInfo<'gc> {
    pub fields: Vec<ClassField<'gc>>,
}

#[derive(Debug)]
pub(crate) enum ValidationError<'gc> {
    ClassNotFound(Token<'gc>),
    MissingFields(Token<'gc>, Vec<&'gc str>),
    InvalidField(Token<'gc>, Token<'gc>), // (class_name, field_token)
    DuplicateField(Token<'gc>, Token<'gc>), // (class_token, field_token)
    TypeError {
        class_token: Token<'gc>,
        field_token: Token<'gc>,
        expected_type: Type<'gc>,
    },
    ComputedPropertyError(Token<'gc>), // class token
}

#[derive(Debug)]
pub(crate) struct TypeResolver<'gc> {
    // Keep track of defined types (classes) in the current scope
    defined_types: HashMap<&'gc str, Type<'gc>>,
    // Track type tokens that need validation
    pending_validations: Vec<Token<'gc>>,
    // Store class information
    class_info: HashMap<&'gc str, ClassInfo<'gc>>,
}

impl Default for TypeResolver<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'gc> TypeResolver<'gc> {
    pub fn new() -> Self {
        let mut resolver = Self {
            defined_types: HashMap::new(),
            pending_validations: Vec::new(),
            class_info: HashMap::new(),
        };

        // Register built-in types
        // In practice, you might want to use string interning for these names
        resolver.defined_types.insert("int", Type::Int);
        resolver.defined_types.insert("str", Type::Str);
        resolver.defined_types.insert("bool", Type::Bool);
        resolver.defined_types.insert("float", Type::Float);

        resolver
    }

    pub fn register_class(&mut self, name: Token<'gc>) {
        self.class_info
            .insert(name.lexeme, ClassInfo { fields: Vec::new() });
    }

    pub fn add_class_field(&mut self, class_name: &'gc str, field: ClassField<'gc>) {
        if let Some(info) = self.class_info.get_mut(class_name) {
            info.fields.push(field);
        }
    }

    // Check the token whether a registerd class or not.
    pub fn check_class(&mut self, token: &Token<'gc>) -> bool {
        self.class_info.contains_key(token.lexeme)
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

    fn check_type(&self, expr: &Expr<'gc>, expected_type: Type<'gc>) -> Result<(), String> {
        match expr {
            // For literals, we can check the type
            Expr::Literal { value, .. } => {
                match (value, expected_type) {
                    (Literal::String(_), Type::Str) |
                    (Literal::Number(_), Type::Int) |
                    (Literal::Number(_), Type::Float)|
                    (Literal::Boolean(_), Type::Bool) |
                    // Nil can be assigned to any type for now
                    (Literal::Nil, _) => Ok(()),
                    _ => Err(format!("Type mismatch: expected {:?}", expected_type)),
                }
            }
            // For variable references, we can't know their exact type at parse time
            // so we accept them (runtime will enforce type safety)
            Expr::Variable { .. } => Ok(()),
            // We'll let runtime handle other cases
            _ => Ok(()),
        }
    }

    pub fn validate_object_literal(
        &self,
        class_name: Token<'gc>,
        properties: &[ObjectProperty<'gc>],
    ) -> Result<(), Vec<ValidationError<'gc>>> {
        let class_info = self
            .class_info
            .get(class_name.lexeme)
            .ok_or_else(|| vec![ValidationError::ClassNotFound(class_name)])?;

        let mut errors = Vec::new();
        let mut provided_fields = HashSet::new();

        // Check each property
        for prop in properties {
            match prop {
                ObjectProperty::Literal { key, value } => {
                    if let Some(field) = class_info
                        .fields
                        .iter()
                        .find(|f| f.name.lexeme == key.lexeme)
                    {
                        if !provided_fields.insert(key.lexeme) {
                            errors.push(ValidationError::DuplicateField(class_name, *key));
                            continue;
                        }
                        // Type check
                        if self.check_type(value, field.ty).is_err() {
                            errors.push(ValidationError::TypeError {
                                class_token: class_name,
                                field_token: *key,
                                expected_type: field.ty,
                            });
                        }
                    } else {
                        errors.push(ValidationError::InvalidField(class_name, *key));
                    }
                    provided_fields.insert(key.lexeme);
                }
                ObjectProperty::Computed { .. } => {
                    errors.push(ValidationError::ComputedPropertyError(class_name));
                }
            }
        }

        // Check for missing required fields
        let missing_fields: Vec<_> = class_info
            .fields
            .iter()
            .filter(|field| field.required && !provided_fields.contains(field.name.lexeme))
            .map(|field| field.name.lexeme)
            .collect();

        if !missing_fields.is_empty() {
            errors.push(ValidationError::MissingFields(class_name, missing_fields));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
