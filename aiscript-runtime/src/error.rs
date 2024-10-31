use aiscript_vm::VmError;
use axum::extract::rejection;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Field validation failed: {field}: {message}")]
    ValidationError { field: String, message: String },

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Type mismatch for field {field}: expected {expected}")]
    TypeMismatch {
        field: String,
        expected: &'static str,
    },

    #[error("Failed to parse JSON body: {0}")]
    JsonParseError(#[from] rejection::JsonRejection),

    #[error("Failed to parse Form body: {0}")]
    FormParseError(#[from] rejection::FormRejection),

    #[error("VM execution error: {0}")]
    VmError(#[from] VmError),
    // #[error("Internal server error: {0}")]
    // InternalError(String),
}
