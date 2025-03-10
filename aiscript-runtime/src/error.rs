use aiscript_vm::VmError;
use axum::{
    Json,
    extract::rejection,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Authentication error: {message}")]
    AuthenticationError { message: String },
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

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        // Convert the error to a JSON object with an "error" field
        let error_json = serde_json::json!({
            "error": self.to_string()
        });

        // Return as a JSON response with BAD_REQUEST status
        (axum::http::StatusCode::BAD_REQUEST, Json(error_json)).into_response()
    }
}
