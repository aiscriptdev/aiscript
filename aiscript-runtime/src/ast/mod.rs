#![allow(unused)]
use std::{borrow::Cow, collections::HashMap};

use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
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
    pub params: Vec<PathParameter>,
}

#[derive(Clone, Debug, Default)]
pub struct RequestBody {
    pub kind: BodyKind,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Default)]
pub enum BodyKind {
    Form,
    #[default]
    Json,
}

#[derive(Clone, Copy, Debug)]
pub enum FieldType {
    Str,
    Number,
    Bool,
    #[allow(unused)]
    Array,
}
impl FieldType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldType::Str => "str",
            FieldType::Number => "number",
            FieldType::Bool => "bool",
            FieldType::Array => "array",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub _type: FieldType,
    pub required: bool,
    pub default: Option<Value>,
    pub directives: Vec<Directive>,
    pub docs: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Directive {
    Simple {
        name: String,
        params: HashMap<String, Value>,
    },
    Any(Vec<Directive>), // Must have 2 or more directives
    Not(Box<Directive>),
    In(Vec<Value>),
}

#[derive(Clone, Debug)]
pub struct Endpoint {
    pub path_specs: Vec<PathSpec>,
    #[allow(unused)]
    pub return_type: Option<String>,
    pub query: Vec<Field>,
    pub body: RequestBody,
    pub statements: String,
    pub docs: String,
}

#[derive(Clone, Debug)]
pub struct Route {
    pub prefix: String,
    pub params: Vec<PathParameter>,
    pub endpoints: Vec<Endpoint>,
    pub docs: String,
}

impl Directive {
    pub fn name(&self) -> Cow<'static, str> {
        match self {
            Directive::Simple { name, .. } => Cow::Owned(name.to_owned()),
            Directive::Any(directives) => "any".into(),
            Directive::Not(directive) => "not".into(),
            Directive::In(values) => "in".into(),
        }
    }
}
