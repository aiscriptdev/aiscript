use std::collections::HashMap;

use serde_json::Value;
use validators::*;

pub(crate) mod validators;

#[derive(Clone, Debug)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Clone)]
pub enum PathSegmentKind {
    Static(String),           // Regular path segment like "users" or "posts"
    Parameter(PathParameter), // Path parameter like "<id:int>"
}

#[derive(Debug, Clone)]
pub struct PathParameter {
    pub name: String,       // Parameter name (e.g., "id")
    pub param_type: String, // Parameter type (e.g., "int")
}

#[derive(Debug, Clone)]
pub struct PathSpec {
    pub method: HttpMethod,
    pub path: String,
    pub path_params: Vec<PathParameter>,
}

#[derive(Clone, Debug)]
pub struct RequestBody {
    pub kind: BodyKind,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug)]
pub enum BodyKind {
    Form,
    Json,
}

#[derive(Clone, Debug)]
pub enum FieldType {
    Str,
    Number,
    Bool,
    Array,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub _type: FieldType,
    pub required: bool,
    pub default: Option<Value>,
    pub validators: Vec<Directive>,
}

// #[derive(Clone, Debug)]
// pub struct Validator {
//     pub kind: ValidatorKind,
//     pub message: Option<String>,
// }

#[derive(Debug, Clone)]
pub enum Directive {
    Simple {
        name: String,
        params: Vec<DirectiveParam>,
    },
    Any(Vec<Directive>), // Must have 2 or more directives
    Not(Box<Directive>),
}

#[derive(Debug, Clone)]
pub enum DirectiveParam {
    Named { name: String, value: Value },
    Positional(Value),
}


#[derive(Clone, Debug)]
pub enum Handler {
    Empty,
    Script,
}

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub path_specs: Vec<PathSpec>,
    pub return_type: Option<String>,
    pub query: Vec<Field>,
    pub body: Option<RequestBody>,
    pub handler: Handler,
}

#[derive(Clone, Debug)]
pub struct Route {
    pub prefix: String,
    pub endpoints: Vec<Endpoint>,
}

// impl Validator {
//     pub fn validate(&self, value: &Value) -> Result<(), String> {
//         match &self.kind {
//             ValidatorKind::Length(length) => length.validate(value),
//             ValidatorKind::Format(format) => format.validate(value),
//         }
//     }
// }
