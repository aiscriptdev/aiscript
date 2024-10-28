use crate::{Field, PathParameter, RequestBody};

// Helper structs for parsing
pub(crate) struct RouteBodyParts {
    pub query: Vec<Field>,
    pub body: RequestBody,
    pub statements: String,
}

pub(crate) enum RoutePartKind {
    Query(Vec<Field>),
    Body(RequestBody),
    // Code(String),
}

#[derive(Debug, Clone)]
pub enum PathSegmentKind {
    Static(String),           // Regular path segment like "users" or "posts"
    Parameter(PathParameter), // Path parameter like "<id:int>"
}
