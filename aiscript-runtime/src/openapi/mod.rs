use aiscript_directive::route::{Auth, RouteAnnotation};
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
            let route_name = route.prefix.trim_matches('/').to_string();

            if !route_name.is_empty() {
                tags.push(Tag {
                    name: route_name.clone(),
                    description: Some(route.docs.clone()),
                    extensions: BTreeMap::new(),
                });
            }

            for endpoint in &route.endpoints {
                for path_spec in &endpoint.path_specs {
                    let path_item = Self::create_path_item(route, endpoint, path_spec);
                    let path = if route.prefix.starts_with('/') {
                        route.prefix.clone()
                    } else {
                        format!("/{}", route.prefix)
                    };
                    let full_path = if path_spec.path.starts_with('/') {
                        format!("{}{}", path, path_spec.path)
                    } else {
                        format!("{}/{}", path, path_spec.path)
                    };
                    paths.insert(full_path.replace("//", "/"), path_item);
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

        security_schemes.insert(
            "jwtAuth".to_string(),
            ObjectOrReference::Object(SecurityScheme::Http {
                description: Some("JWT Authentication".to_string()),
                scheme: "bearer".to_string(),
                bearer_format: Some("JWT".to_string()),
            }),
        );

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
            summary: Some(endpoint.docs.clone()),
            parameters: Self::create_path_parameters(&path_spec.params),
            ..Default::default()
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

    #[allow(unused)]
    fn get_security_requirement(
        annotation: RouteAnnotation,
    ) -> Option<Vec<BTreeMap<String, Vec<String>>>> {
        match annotation.auth {
            Auth::Jwt => {
                let mut requirement = BTreeMap::new();
                requirement.insert("jwtAuth".to_string(), vec![]);
                Some(vec![requirement])
            }
            Auth::Basic => {
                let mut requirement = BTreeMap::new();
                requirement.insert("basicAuth".to_string(), vec![]);
                Some(vec![requirement])
            }
            Auth::None => None,
        }
    }

    fn create_operation(route: &Route, endpoint: &Endpoint, path_spec: &PathSpec) -> Operation {
        let route_name = route.prefix.trim_matches('/').to_string();
        let tags = if route_name.is_empty() {
            vec!["default".to_string()]
        } else {
            vec![route_name]
        };

        let mut parameters = Self::create_path_parameters(&path_spec.params);
        for query_field in &endpoint.query {
            parameters.push(Self::create_query_parameter(query_field));
        }

        let request_body = if !endpoint.body.fields.is_empty() {
            Some(Self::create_request_body(&endpoint.body))
        } else {
            None
        };

        let method = path_spec.method.as_str().to_lowercase();
        let path = path_spec.path.trim_matches('/');
        let operation_id = if path.is_empty() {
            method
        } else {
            format!("{}_{}", method, path.replace('/', "_"))
        };

        Operation {
            tags,
            summary: Some(endpoint.docs.clone()),
            operation_id: Some(operation_id),
            parameters,
            request_body,
            responses: Some(Self::create_default_responses()),
            // security: Self::get_security_requirement(endpoint.annotation),
            ..Default::default()
        }
    }

    fn create_path_parameters(params: &[PathParameter]) -> Vec<ObjectOrReference<Parameter>> {
        params
            .iter()
            .map(|param| {
                ObjectOrReference::Object(Parameter {
                    name: param.name.clone(),
                    location: ParameterIn::Path,
                    description: Some(String::new()),
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

        let mut content = BTreeMap::new();
        content.insert(
            content_type.to_string(),
            MediaType {
                schema: Some(ObjectOrReference::Object(ObjectSchema {
                    properties,
                    required,
                    schema_type: Some(SchemaTypeSet::Single(Type::Object)),
                    ..Default::default()
                })),
                ..Default::default()
            },
        );

        ObjectOrReference::Object(RequestBody {
            content,
            required: Some(true),
            ..Default::default()
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
            ..Default::default()
        };

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
            description: Some(String::new()),
            ..Default::default()
        })
    }

    fn create_default_responses() -> BTreeMap<String, ObjectOrReference<Response>> {
        let mut responses = BTreeMap::new();
        responses.insert(
            "200".to_string(),
            ObjectOrReference::Object(Response {
                description: Some("Successful operation".to_string()),
                ..Default::default()
            }),
        );
        responses
    }
}
