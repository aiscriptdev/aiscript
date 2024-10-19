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

#[derive(Clone, Debug)]
pub struct HttpRoute {
    pub path: String,
}

#[derive(Clone, Debug)]
pub struct Signature {
    pub method: HttpMethod,
    pub route: HttpRoute,
}

#[derive(Clone, Debug)]
pub struct Body {
    pub kind: BodyKind,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug)]
pub enum BodyKind {
    Form,
    Json,
}

#[derive(Clone, Debug)]
pub struct Data {
    pub fields: Vec<Field>,
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
    pub validators: Vec<Validator>,
}

#[derive(Clone, Debug)]
pub struct Validator {
    pub kind: ValidatorKind,
    pub message: String,
}

#[derive(Clone, Debug)]
pub enum Handler {
    Empty,
    Dsl,
    Python,
}
#[derive(Clone, Debug)]
pub enum DslHandler {}

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub signatures: Vec<Signature>,
    pub headers: HashMap<String, String>,
    pub query: Vec<Field>,
    pub body: Option<Body>,
    pub handler: Handler,
}

#[derive(Clone, Debug)]
pub struct Route {
    pub prefix: String,
    pub endpoints: Vec<Endpoint>,
}

impl Validator {
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        match &self.kind {
            ValidatorKind::Length(length) => length.validate(value),
            ValidatorKind::Format(format) => format.validate(value),
        }
    }
}
