mod generate;

pub use generate::OpenAPIGenerator;
use indexmap::IndexMap;
use serde::Serialize;

use crate::ast::HttpMethod;

#[derive(Serialize)]
pub struct OpenAPI {
    openapi: String,
    info: Info,
    paths: IndexMap<String, PathItem>,
    components: Components,
}

#[derive(Serialize)]
struct Info {
    title: String,
    version: String,
}

#[derive(Serialize, Default)]
struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    post: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    put: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delete: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Vec<Parameter>>,
}

#[derive(Serialize)]
struct Operation {
    summary: String,
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Vec<Parameter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_body: Option<RequestBodySpec>,
    responses: IndexMap<String, Response>,
}

#[derive(Serialize)]
struct Parameter {
    name: String,
    #[serde(rename = "in")]
    location: String,
    description: Option<String>,
    required: bool,
    schema: Schema,
}

#[derive(Serialize)]
struct RequestBodySpec {
    description: Option<String>,
    required: bool,
    content: IndexMap<String, MediaType>,
}

#[derive(Serialize)]
struct MediaType {
    schema: Schema,
}

#[derive(Serialize)]
struct Response {
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<IndexMap<String, MediaType>>,
}

#[derive(Serialize)]
struct Components {
    schemas: IndexMap<String, Schema>,
}

#[derive(Serialize, Default)]
struct Schema {
    #[serde(rename = "type")]
    schema_type: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<IndexMap<String, Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<serde_json::Value>,
    // Validation
    #[serde(skip_serializing_if = "Option::is_none")]
    minimum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maximum: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enum_values: Option<Vec<String>>,
    #[serde(rename = "$ref", skip_serializing_if = "Option::is_none")]
    reference: Option<String>,
}

#[allow(unused)]
impl Schema {
    fn new(schema_type: &'static str) -> Schema {
        Schema {
            schema_type,
            ..Default::default()
        }
    }

    fn format(mut self, format: &'static str) -> Schema {
        self.format = Some(format);
        self
    }

    fn properties(mut self, properties: IndexMap<String, Schema>) -> Schema {
        self.properties = Some(properties);
        self
    }

    fn required(mut self, required: Vec<String>) -> Schema {
        self.required = if required.is_empty() {
            None
        } else {
            Some(required)
        };
        self
    }

    fn default(mut self, default: serde_json::Value) -> Schema {
        self.default = Some(default);
        self
    }

    fn minimum(mut self, minimum: f64) -> Schema {
        self.minimum = Some(minimum);
        self
    }

    fn maximum(mut self, maximum: f64) -> Schema {
        self.maximum = Some(maximum);
        self
    }

    fn min_length(mut self, min_length: u64) -> Schema {
        self.min_length = Some(min_length);
        self
    }

    fn max_length(mut self, max_length: u64) -> Schema {
        self.max_length = Some(max_length);
        self
    }

    fn pattern(mut self, pattern: String) -> Schema {
        self.pattern = Some(pattern);
        self
    }

    fn enum_values(mut self, enum_values: Vec<String>) -> Schema {
        self.enum_values = Some(enum_values);
        self
    }

    fn reference(mut self, reference: String) -> Schema {
        self.reference = Some(reference);
        self
    }
}

impl PathItem {
    fn add_operation(&mut self, method: HttpMethod, operation: Operation) {
        match method {
            HttpMethod::Get => self.get = Some(operation),
            HttpMethod::Post => self.post = Some(operation),
            HttpMethod::Put => self.put = Some(operation),
            HttpMethod::Delete => self.delete = Some(operation),
        }
    }
}
