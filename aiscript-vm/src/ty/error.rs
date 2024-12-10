// Exprimental error type checker
#![allow(unused)]
use aiscript_lexer::TokenType;

use crate::lexer::Token;
use std::collections::HashSet;

#[derive(Debug, Hash, Eq, PartialEq)]
struct ErrorType<'gc> {
    name: &'gc str,
    kind: TokenType,
}

#[derive(Debug)]
pub struct FunctionErrorResolver<'gc> {
    // Declared error types in function signature
    declared_errors: HashSet<ErrorType<'gc>>,
    // Actually raised error types in function body
    raised_errors: HashSet<ErrorType<'gc>>,
    // Current function name for error reporting
    function_name: Token<'gc>,
    // Track if we're inside a function that can raise errors
    pub(crate) in_error_function: bool,
}

impl<'gc> FunctionErrorResolver<'gc> {
    pub fn new(function_name: Token<'gc>) -> Self {
        Self {
            declared_errors: HashSet::new(),
            raised_errors: HashSet::new(),
            function_name,
            in_error_function: false,
        }
    }

    pub fn add_declared_error(&mut self, error: Token<'gc>) {
        self.declared_errors.insert(ErrorType {
            name: error.lexeme,
            kind: error.kind,
        });
        self.in_error_function = true;
    }

    pub fn add_raised_error(&mut self, error: Token<'gc>) {
        self.raised_errors.insert(ErrorType {
            name: error.lexeme,
            kind: error.kind,
        });
    }

    pub fn validate(&self) -> Result<(), String> {
        // Check for undeclared raised errors
        for error in &self.raised_errors {
            if !self.declared_errors.contains(error) {
                return Err(format!(
                    "Error type '{}' is raised but not declared in function '{}'s signature",
                    error.name, self.function_name.lexeme
                ));
            }
        }

        // Check for unused declared errors
        for error in &self.declared_errors {
            if !self.raised_errors.contains(error) {
                return Err(format!(
                    "Error type '{}' is declared but never raised in function '{}'",
                    error.name, self.function_name.lexeme
                ));
            }
        }

        Ok(())
    }

    pub fn has_error_types(&self) -> bool {
        !self.declared_errors.is_empty()
    }
}
