use super::*;
use crate::ast::{
    BodyKind, Directive, Endpoint, Field, FieldType, PathParameter, RequestBody, Route,
};

pub struct OpenAPIGenerator;

impl OpenAPIGenerator {
    pub fn generate(routes: &[Route]) -> OpenAPI {
        let mut paths = IndexMap::new();
        let mut components = Components {
            schemas: IndexMap::new(),
        };

        // First, collect all unique request body types
        for route in routes {
            for endpoint in &route.endpoints {
                if !endpoint.body.fields.is_empty() {
                    let schema_name =
                        Self::generate_schema_name(&route.prefix, &endpoint.path_specs[0].path);
                    components.schemas.insert(
                        schema_name.clone(),
                        Self::create_body_schema(&endpoint.body),
                    );
                }
            }
        }

        // Then process routes
        for route in routes {
            Self::process_route(route, &mut paths, &components);
        }

        OpenAPI {
            openapi: "3.0.3".to_string(),
            info: Info {
                title: "Generated API".to_string(),
                version: "1.0.0".to_string(),
            },
            paths,
            components,
        }
    }

    fn generate_schema_name(prefix: &str, path: &str) -> String {
        let clean_path = format!("{}{}", prefix, path)
            .replace('/', "_")
            .trim_matches('_')
            .to_string();
        format!("{}Request", clean_path)
    }

    fn create_body_schema(body: &RequestBody) -> Schema {
        let mut properties = IndexMap::new();
        let mut required = Vec::new();

        for field in &body.fields {
            properties.insert(
                field.name.clone(),
                Self::field_type_to_schema_with_directives(field),
            );
            if field.required {
                required.push(field.name.clone());
            }
        }

        Schema::new("object")
            .properties(properties)
            .required(required)
    }

    fn create_request_body(body: &RequestBody, schema_name: &str) -> RequestBodySpec {
        let content_type = match body.kind {
            BodyKind::Json => "application/json",
            BodyKind::Form => "application/x-www-form-urlencoded",
        };

        let mut content = IndexMap::new();
        content.insert(
            content_type.to_string(),
            MediaType {
                schema: Schema::new("object")
                    .reference(format!("#/components/schemas/{}", schema_name)),
            },
        );

        RequestBodySpec {
            description: None,
            required: true,
            content,
        }
    }

    fn process_route(
        route: &Route,
        paths: &mut IndexMap<String, PathItem>,
        components: &Components,
    ) {
        for endpoint in &route.endpoints {
            for path_spec in &endpoint.path_specs {
                let full_path = format!("{}{}", route.prefix, path_spec.path);
                let operation =
                    Self::create_operation(endpoint, &route.params, &full_path, components);

                paths
                    .entry(full_path)
                    .or_default()
                    .add_operation(path_spec.method.clone(), operation);
            }
        }
    }

    fn create_operation(
        endpoint: &Endpoint,
        route_params: &[PathParameter],
        full_path: &str,
        components: &Components,
    ) -> Operation {
        let mut parameters = Vec::new();

        // Add path parameters
        for param in route_params {
            parameters.push(Parameter {
                name: param.name.clone(),
                location: "path".to_string(),
                description: None,
                required: true,
                schema: Self::field_type_to_schema(&param.param_type),
            });
        }

        // Add query parameters
        for field in &endpoint.query {
            parameters.push(Self::field_to_parameter(field));
        }

        // Create request body if needed
        let request_body = if !endpoint.body.fields.is_empty() {
            let schema_name =
                Self::generate_schema_name(full_path.split('/').next().unwrap_or(""), full_path);
            Some(Self::create_request_body(&endpoint.body, &schema_name))
        } else {
            None
        };

        Operation {
            summary: endpoint.docs.clone(),
            description: Some(endpoint.docs.clone()),
            parameters: Some(parameters),
            request_body,
            responses: Self::default_responses(),
        }
    }

    fn field_type_to_schema(type_str: &str) -> Schema {
        match type_str {
            "int" => Schema::new("integer").format("int32"),
            "bool" => Schema::new("boolean"),
            _ => Schema::new("string"),
        }
    }

    fn field_to_parameter(field: &Field) -> Parameter {
        let mut schema = Self::field_type_to_schema_with_directives(field);

        if let Some(default) = &field.default {
            schema.default = Some(default.clone());
        }

        Parameter {
            name: field.name.clone(),
            location: "query".to_string(),
            description: Some(field.docs.clone()),
            required: field.required,
            schema,
        }
    }

    fn field_type_to_schema_with_directives(field: &Field) -> Schema {
        let mut schema = match field._type {
            FieldType::Str => Schema::new("string"),
            FieldType::Number => Schema::new("number"),
            FieldType::Bool => Schema::new("boolean"),
            FieldType::Array => Schema::new("array"),
        };

        // Process directives
        for directive in &field.directives {
            Self::apply_directive(&mut schema, directive);
        }

        schema
    }

    fn apply_directive(schema: &mut Schema, directive: &Directive) {
        match directive {
            Directive::Simple { name, params } => {
                if let "string" = name.as_str() {
                    if let Some(min_len) = params.get("min_len") {
                        if let Some(val) = min_len.as_u64() {
                            schema.min_length = Some(val);
                        }
                    }
                    if let Some(max_len) = params.get("max_len") {
                        if let Some(val) = max_len.as_u64() {
                            schema.max_length = Some(val);
                        }
                    }
                }
            }
            Directive::In(values) => {
                schema.enum_values = Some(
                    values
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect(),
                );
            }
            _ => {}
        }
    }

    fn default_responses() -> IndexMap<String, Response> {
        let mut responses = IndexMap::new();
        responses.insert(
            "200".to_string(),
            Response {
                description: "Successful operation".to_string(),
                content: None,
            },
        );
        responses
    }
}
