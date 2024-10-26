use crate::{Field, RequestBody};

// Helper structs for parsing
pub(crate) struct RouteBodyParts {
    pub query: Vec<Field>,
    pub body: Option<RequestBody>,
    // handler_code: String,
}

pub(crate) enum RoutePartKind {
    Query(Vec<Field>),
    Body(RequestBody),
    // Code(String),
}
