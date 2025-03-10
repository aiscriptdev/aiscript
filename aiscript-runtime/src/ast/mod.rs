#![allow(unused)]
use aiscript_directive::{Validator, route::RouteAnnotation};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
        }
    }
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

#[derive(Debug, Default)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

pub struct Field {
    pub name: String,
    pub _type: FieldType,
    pub required: bool,
    pub default: Option<Value>,
    pub validators: Box<[Box<dyn Validator>]>,
    pub docs: String,
}

impl std::fmt::Debug for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field")
            .field("name", &self.name)
            .field("_type", &self._type)
            .field("required", &self.required)
            .field("default", &self.default)
            .field("docs", &self.docs)
            .finish()
    }
}

#[derive(Debug)]
pub struct Endpoint {
    pub annotation: RouteAnnotation,
    pub path_specs: Vec<PathSpec>,
    #[allow(unused)]
    pub return_type: Option<String>,
    pub path: Vec<Field>,
    pub query: Vec<Field>,
    pub body: RequestBody,
    pub statements: String,
    pub docs: String,
}

#[derive(Debug)]
pub struct Route {
    pub annotation: RouteAnnotation,
    pub prefix: String,
    pub params: Vec<PathParameter>,
    pub endpoints: Vec<Endpoint>,
    pub docs: String,
}
