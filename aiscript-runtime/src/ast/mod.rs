#![allow(unused)]
use std::{borrow::Cow, collections::HashMap};

use aiscript_directive::{Directive, Validator};
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

#[derive(Default)]
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

pub struct Field {
    pub name: String,
    pub _type: FieldType,
    pub required: bool,
    pub default: Option<Value>,
    pub validators: Box<[Box<dyn Validator>]>,
    pub docs: String,
}

#[derive(Debug, Copy, Clone)]
pub enum Auth {
    Jwt,
    Basic,
    None,
}

impl Auth {
    pub fn is_required(&self) -> bool {
        match self {
            Auth::Jwt | Auth::Basic => true,
            Auth::None => false,
        }
    }

    pub fn or(self, other: Auth) -> Auth {
        match self {
            Auth::None => other,
            _ => self,
        }
    }
}

pub struct Endpoint {
    pub auth: Auth,
    pub path_specs: Vec<PathSpec>,
    #[allow(unused)]
    pub return_type: Option<String>,
    pub query: Vec<Field>,
    pub body: RequestBody,
    pub statements: String,
    pub docs: String,
}

pub struct Route {
    pub auth: Auth,
    pub prefix: String,
    pub params: Vec<PathParameter>,
    pub endpoints: Vec<Endpoint>,
    pub docs: String,
}
