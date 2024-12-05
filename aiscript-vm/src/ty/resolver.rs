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
    MissingFields(Vec<&'gc str>),
    InvalidFields(Vec<(&'gc str, u32)>), // (field_name, line_number)
    TypeError {
        field: &'gc str,
        line: u32,
        message: String,
    },
    ComputedPropertyError(u32), // line number
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

impl<'gc> Default for TypeResolver<'gc> {
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
        let class_info = self.class_info.get(class_name.lexeme).ok_or_else(|| {
            vec![ValidationError::TypeError {
                field: class_name.lexeme,
                line: class_name.line,
                message: format!("Class '{}' not found", class_name.lexeme),
            }]
        })?;

        let mut errors = Vec::new();
        let mut provided_fields = HashSet::new();

        // Handle empty object case
        if properties.is_empty() {
            let missing_fields: Vec<_> = class_info
                .fields
                .iter()
                .filter(|field| field.required)
                .map(|field| field.name.lexeme)
                .collect();

            if !missing_fields.is_empty() {
                errors.push(ValidationError::MissingFields(missing_fields));
            }
            return if errors.is_empty() {
                Ok(())
            } else {
                Err(errors)
            };
        }

        // Check each property
        for prop in properties {
            match prop {
                ObjectProperty::Literal { key, value } => {
                    if let Some(field) = class_info
                        .fields
                        .iter()
                        .find(|f| f.name.lexeme == key.lexeme)
                    {
                        // Type check
                        if let Err(message) = self.check_type(value, field.ty) {
                            errors.push(ValidationError::TypeError {
                                field: key.lexeme,
                                line: key.line,
                                message,
                            });
                        }
                    } else {
                        errors.push(ValidationError::InvalidFields(vec![(key.lexeme, key.line)]));
                    }
                    provided_fields.insert(key.lexeme);
                }
                ObjectProperty::Computed { key_expr, .. } => {
                    // Use the line number from the expression
                    errors.push(ValidationError::ComputedPropertyError(key_expr.line()));
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
            errors.push(ValidationError::MissingFields(missing_fields));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
