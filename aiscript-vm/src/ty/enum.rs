use std::collections::HashSet;

use crate::{ast::Literal, lexer::Token};
#[derive(Debug, Clone, Copy, PartialEq)]
enum EnumValueType {
    Integer,
    String,
    Boolean,
    Unset,
}

pub(crate) struct EnumVariantChecker<'gc> {
    value_type: EnumValueType,
    used_values: HashSet<Literal<'gc>>,
    next_int_value: f64,
}

impl<'gc> EnumVariantChecker<'gc> {
    pub(crate) fn new() -> Self {
        Self {
            value_type: EnumValueType::Unset,
            used_values: HashSet::new(),
            next_int_value: 0.0,
        }
    }

    pub(crate) fn check_value(
        &mut self,
        variant_name: Token<'gc>,
        literal: &Literal<'gc>,
    ) -> Result<(), String> {
        // Check value type
        match (self.value_type, literal) {
            // First value sets the type
            (EnumValueType::Unset, Literal::Number(n)) => {
                self.value_type = EnumValueType::Integer;
                self.next_int_value = *n + 1.0;
            }
            (EnumValueType::Unset, Literal::String(_)) => {
                self.value_type = EnumValueType::String;
            }
            (EnumValueType::Unset, Literal::Boolean(_)) => {
                self.value_type = EnumValueType::Boolean;
            }

            // Check type consistency
            (EnumValueType::Integer, Literal::Number(n)) => {
                if *n < self.next_int_value {
                    return Err(format!(
                        "Enum variant '{}' value {} must be greater than or equal to {} (next auto-increment value)",
                        variant_name.lexeme, n, self.next_int_value
                    ));
                }
                self.next_int_value = *n + 1.0;
            }
            (EnumValueType::String, Literal::String(_)) => {}
            (EnumValueType::Boolean, Literal::Boolean(_)) => {}

            // Type mismatch errors
            (expected, _) => {
                let type_name = match expected {
                    EnumValueType::Integer => "integer",
                    EnumValueType::String => "string",
                    EnumValueType::Boolean => "boolean",
                    EnumValueType::Unset => unreachable!(),
                };
                return Err(format!(
                    "Enum variant '{}' must be of type {}",
                    variant_name.lexeme, type_name
                ));
            }
        }

        // Check for duplicates
        if self.used_values.contains(literal) {
            return Err(format!(
                "Duplicate value {:?} in enum variant '{}'",
                literal, variant_name.lexeme
            ));
        }

        self.used_values.insert(*literal);
        Ok(())
    }

    pub(crate) fn next_value(&mut self) -> Option<Literal<'gc>> {
        match self.value_type {
            EnumValueType::Integer => {
                let value = Literal::Number(self.next_int_value);
                self.used_values.insert(value);
                self.next_int_value += 1.0;
                Some(value)
            }
            _ => None,
        }
    }

    pub(crate) fn is_auto_increment_supported(&self) -> bool {
        matches!(
            self.value_type,
            EnumValueType::Integer | EnumValueType::Unset
        )
    }
}
