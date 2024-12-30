use aiscript_directive::validator::{InValidator, StringValidator};
use aiscript_directive::Validator;
use oas3::{
    spec::{
        Components, Info, MediaType, ObjectOrReference, ObjectSchema, Operation, Parameter,
        ParameterIn, ParameterStyle, PathItem, RequestBody, Response, SchemaType as Type,
        SchemaTypeSet, SecurityScheme, Server, Tag,
    },
    Spec,
};
use std::collections::BTreeMap;

use crate::ast::{
    BodyKind, Endpoint, Field, FieldType, HttpMethod, PathParameter, PathSpec, Route,
};

pub struct OpenAPIGenerator;

impl OpenAPIGenerator {
    pub fn generate(routes: &[Route]) -> Spec {
        let mut paths = BTreeMap::new();
        let mut tags = Vec::new();

        for route in routes {
            // Create a tag for the route
            tags.push(Tag {
                name: route.prefix.trim_matches('/').to_string(),
                description: Some(route.docs.clone()),
                extensions: BTreeMap::new(),
            });

            // Process endpoints for this route
            for endpoint in &route.endpoints {
                for path_spec in &endpoint.path_specs {
                    let path_item = Self::create_path_item(route, endpoint, path_spec);
                    let full_path = format!("{}{}", route.prefix, path_spec.path);
                    paths.insert(full_path, path_item);
                }
            }
        }

        Spec {
            openapi: "3.0.3".to_string(),
            info: Info {
                title: "API Documentation".to_string(),
                summary: None,
                description: Some("API Documentation generated from routes".to_string()),
                terms_of_service: None,
                version: "1.0.0".to_string(),
                contact: None,
                license: None,
                extensions: BTreeMap::new(),
            },
            servers: vec![Server {
                url: "/".to_string(),
                description: None,
                variables: BTreeMap::new(),
            }],
            paths: Some(paths),
            components: Some(Self::create_default_components()),
            tags,
            webhooks: BTreeMap::new(),
            external_docs: None,
            extensions: BTreeMap::new(),
        }
    }

    fn create_default_components() -> Components {
        let mut security_schemes = BTreeMap::new();

        // Add JWT auth scheme
        security_schemes.insert(
            "jwtAuth".to_string(),
            ObjectOrReference::Object(SecurityScheme::Http {
                description: Some("JWT Authentication".to_string()),
                scheme: "bearer".to_string(),
                bearer_format: Some("JWT".to_string()),
            }),
        );

        // Add Basic auth scheme
        security_schemes.insert(
            "basicAuth".to_string(),
            ObjectOrReference::Object(SecurityScheme::Http {
                description: Some("Basic Authentication".to_string()),
                scheme: "basic".to_string(),
                bearer_format: None,
            }),
        );

        Components {
            schemas: BTreeMap::new(),
            responses: BTreeMap::new(),
            parameters: BTreeMap::new(),
            examples: BTreeMap::new(),
            request_bodies: BTreeMap::new(),
            headers: BTreeMap::new(),
            path_items: BTreeMap::new(),
            security_schemes,
            links: BTreeMap::new(),
            callbacks: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }

    fn create_path_item(route: &Route, endpoint: &Endpoint, path_spec: &PathSpec) -> PathItem {
        let mut path_item = PathItem {
            reference: None,
            summary: None,
            description: Some(endpoint.docs.clone()),
            get: None,
            put: None,
            post: None,
            delete: None,
            options: None,
            head: None,
            patch: None,
            trace: None,
            servers: vec![],
            parameters: Self::create_path_parameters(&path_spec.params),
            extensions: BTreeMap::new(),
        };

        let operation = Self::create_operation(route, endpoint, path_spec);

        match path_spec.method {
            HttpMethod::Get => path_item.get = Some(operation),
            HttpMethod::Post => path_item.post = Some(operation),
            HttpMethod::Put => path_item.put = Some(operation),
            HttpMethod::Delete => path_item.delete = Some(operation),
        }

        path_item
    }

    fn create_operation(route: &Route, endpoint: &Endpoint, path_spec: &PathSpec) -> Operation {
        let tag = route.prefix.trim_matches('/').to_string();
        let mut parameters = Self::create_path_parameters(&path_spec.params);

        // Add query parameters
        for query_field in &endpoint.query {
            parameters.push(Self::create_query_parameter(query_field));
        }

        let request_body = if !endpoint.body.fields.is_empty() {
            Some(Self::create_request_body(&endpoint.body))
        } else {
            None
        };

        Operation {
            tags: vec![tag],
            summary: None,
            description: Some(endpoint.docs.clone()),
            external_docs: None,
            operation_id: Some(format!(
                "{}_{}",
                path_spec.method.as_str().to_lowercase(),
                path_spec.path.replace("/", "_")
            )),
            parameters,
            request_body,
            responses: Some(Self::create_default_responses()),
            callbacks: BTreeMap::new(),
            deprecated: None,
            servers: vec![],
            extensions: BTreeMap::new(),
        }
    }

    fn create_path_parameters(params: &[PathParameter]) -> Vec<ObjectOrReference<Parameter>> {
        params
            .iter()
            .map(|param| {
                ObjectOrReference::Object(Parameter {
                    name: param.name.clone(),
                    location: ParameterIn::Path,
                    description: None,
                    required: Some(true),
                    deprecated: None,
                    allow_empty_value: None,
                    style: Some(ParameterStyle::Simple),
                    explode: None,
                    allow_reserved: None,
                    schema: Some(Self::create_schema_for_type(&param.param_type)),
                    example: None,
                    examples: BTreeMap::new(),
                    content: None,
                    extensions: BTreeMap::new(),
                })
            })
            .collect()
    }

    fn create_query_parameter(field: &Field) -> ObjectOrReference<Parameter> {
        ObjectOrReference::Object(Parameter {
            name: field.name.clone(),
            location: ParameterIn::Query,
            description: Some(field.docs.clone()),
            required: Some(field.required),
            deprecated: None,
            allow_empty_value: None,
            style: Some(ParameterStyle::Form),
            explode: None,
            allow_reserved: None,
            schema: Some(Self::create_schema_for_field(field)),
            example: field.default.clone(),
            examples: BTreeMap::new(),
            content: None,
            extensions: BTreeMap::new(),
        })
    }

    fn create_request_body(body: &crate::ast::RequestBody) -> ObjectOrReference<RequestBody> {
        let mut properties = BTreeMap::new();
        let mut required = Vec::new();

        for field in &body.fields {
            properties.insert(field.name.clone(), Self::create_schema_for_field(field));
            if field.required {
                required.push(field.name.clone());
            }
        }

        let content_type = match body.kind {
            BodyKind::Json => "application/json",
            BodyKind::Form => "application/x-www-form-urlencoded",
        };

        let schema = ObjectOrReference::Object(ObjectSchema {
            properties,
            required,
            schema_type: Some(SchemaTypeSet::Single(Type::Object)),
            ..Self::create_default_schema()
        });

        let mut content = BTreeMap::new();
        content.insert(
            content_type.to_string(),
            MediaType {
                schema: Some(schema),
                examples: None,
                encoding: BTreeMap::new(),
            },
        );

        ObjectOrReference::Object(RequestBody {
            description: None,
            content,
            required: Some(true),
        })
    }

    fn create_schema_for_field(field: &Field) -> ObjectOrReference<ObjectSchema> {
        let mut schema = ObjectSchema {
            schema_type: Some(SchemaTypeSet::Single(match field._type {
                FieldType::Str => Type::String,
                FieldType::Number => Type::Number,
                FieldType::Bool => Type::Boolean,
                FieldType::Array => Type::Array,
            })),
            description: Some(field.docs.clone()),
            default: field.default.clone(),
            ..Self::create_default_schema()
        };

        // Process validators
        for validator in field.validators.iter() {
            if let Some(string_validator) = validator.downcast_ref::<StringValidator>() {
                if let Some(min_len) = string_validator.min_len {
                    schema.min_length = Some(min_len as u64);
                }
                if let Some(max_len) = string_validator.max_len {
                    schema.max_length = Some(max_len as u64);
                }
            }
            if let Some(in_validator) = validator.downcast_ref::<InValidator>() {
                schema.enum_values = in_validator.0.clone();
            }
            // Add other validator types as needed
        }

        ObjectOrReference::Object(schema)
    }

    fn create_schema_for_type(type_str: &str) -> ObjectOrReference<ObjectSchema> {
        ObjectOrReference::Object(ObjectSchema {
            schema_type: Some(SchemaTypeSet::Single(match type_str {
                "str" => Type::String,
                "int" => Type::Integer,
                "float" => Type::Number,
                "bool" => Type::Boolean,
                _ => Type::String,
            })),
            ..Self::create_default_schema()
        })
    }

    fn create_default_responses() -> BTreeMap<String, ObjectOrReference<Response>> {
        let mut responses = BTreeMap::new();

        // Default 200 response
        responses.insert(
            "200".to_string(),
            ObjectOrReference::Object(Response {
                description: Some("Successful operation".to_string()),
                headers: BTreeMap::new(),
                content: BTreeMap::new(),
                links: BTreeMap::new(),
                extensions: BTreeMap::new(),
            }),
        );

        responses
    }

    fn create_default_schema() -> ObjectSchema {
        ObjectSchema {
            all_of: vec![],
            any_of: vec![],
            one_of: vec![],
            items: None,
            properties: BTreeMap::new(),
            additional_properties: None,
            schema_type: None,
            enum_values: vec![],
            const_value: None,
            multiple_of: None,
            maximum: None,
            exclusive_maximum: None,
            minimum: None,
            exclusive_minimum: None,
            max_length: None,
            min_length: None,
            pattern: None,
            max_items: None,
            min_items: None,
            unique_items: None,
            max_properties: None,
            min_properties: None,
            required: vec![],
            format: None,
            title: None,
            description: None,
            default: None,
            deprecated: None,
            read_only: None,
            write_only: None,
            examples: vec![],
            discriminator: None,
            example: None,
            extensions: BTreeMap::new(),
        }
    }
}
