use crate::template;
use aiscript_directive::{Validator, route::RouteAnnotation};
use aiscript_vm::{ReturnValue, Vm, VmError};
use axum::{
    Form, Json, RequestExt,
    body::Body,
    extract::{self, FromRequest, RawPathParams, Request},
    http::{HeaderName, HeaderValue},
    response::{IntoResponse, Response},
};
use axum_extra::{
    TypedHeader,
    headers::{
        Authorization,
        authorization::{Basic, Bearer},
    },
};
use hyper::StatusCode;
use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode};
use serde_json::Value;
use sqlx::{PgPool, SqlitePool};
use std::{
    collections::HashMap,
    convert::Infallible,
    future::Future,
    mem,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::task::{self, JoinHandle};
use tower::Service;

use crate::{
    Config,
    ast::{self, *},
};

use crate::error::ServerError;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

#[derive(Clone)]
pub struct Field {
    name: String,
    field_type: FieldType,
    required: bool,
    default: Option<Value>,
    validators: Arc<[Box<dyn Validator>]>,
}

#[derive(Clone)]
pub struct Endpoint {
    pub annotation: RouteAnnotation,
    pub path_params: Vec<Field>,
    pub query_params: Vec<Field>,
    pub body_type: BodyKind,
    pub body_fields: Vec<Field>,
    pub script: String,
    pub path_specs: Vec<PathSpec>,
    // pub provider_manager: Arc<ProviderManager>,
    pub pg_connection: Option<PgPool>,
    pub sqlite_connection: Option<SqlitePool>,
    pub redis_connection: Option<redis::aio::MultiplexedConnection>,
}

enum ProcessingState {
    ValidatingAuth,
    ValidatingPath,
    ValidatingQuery,
    ValidatingBody,
    Executing(JoinHandle<Result<ReturnValue, VmError>>),
}

pub struct RequestProcessor {
    endpoint: Endpoint,
    request: Request<Body>,
    jwt_claim: Option<Value>,
    path_data: HashMap<String, Value>,
    query_data: HashMap<String, Value>,
    body_data: HashMap<String, Value>,
    state: ProcessingState,
}

impl RequestProcessor {
    fn new(endpoint: Endpoint, request: Request<Body>) -> Self {
        let state = if endpoint.annotation.is_auth_required() {
            ProcessingState::ValidatingAuth
        } else {
            ProcessingState::ValidatingPath
        };
        Self {
            endpoint,
            request,
            jwt_claim: None,
            path_data: HashMap::new(),
            query_data: HashMap::new(),
            body_data: HashMap::new(),
            state,
        }
    }

    fn validate_field(field: &Field, value: &Value) -> Result<Value, ServerError> {
        // Try to convert the value if it doesn't match the expected type
        let converted_value = match (field.field_type, value) {
            // Already correct types
            (FieldType::Str, Value::String(_))
            | (FieldType::Number, Value::Number(_))
            | (FieldType::Bool, Value::Bool(_))
            | (FieldType::Array, Value::Array(_)) => value.clone(),

            // Convert string to number
            (FieldType::Number, Value::String(s)) => {
                // Try parsing as integer first
                if let Ok(n) = s.parse::<i64>() {
                    Value::Number(serde_json::Number::from(n))
                }
                // Then try as floating point
                else if let Ok(f) = s.parse::<f64>() {
                    match serde_json::Number::from_f64(f) {
                        Some(n) => Value::Number(n),
                        None => {
                            return Err(ServerError::TypeMismatch {
                                field: field.name.clone(),
                                expected: field.field_type.as_str(),
                            });
                        }
                    }
                }
                // Cannot parse as number
                else {
                    return Err(ServerError::TypeMismatch {
                        field: field.name.clone(),
                        expected: field.field_type.as_str(),
                    });
                }
            }

            // Convert string to boolean
            (FieldType::Bool, Value::String(s)) => match s.to_lowercase().as_str() {
                "true" /*| "yes" | "1" | "on"*/ => Value::Bool(true),
                "false" /*| "no" | "0" | "off"*/ => Value::Bool(false),
                _ => {
                    return Err(ServerError::TypeMismatch {
                        field: field.name.clone(),
                        expected: field.field_type.as_str(),
                    })
                }
            },

            // Convert number to boolean (0 = false, non-zero = true)
            (FieldType::Bool, Value::Number(n)) => {
                if n.as_f64().unwrap_or(0.0) == 0.0 {
                    Value::Bool(false)
                } else {
                    Value::Bool(true)
                }
            }

            // Convert number to string
            (FieldType::Str, Value::Number(n)) => Value::String(n.to_string()),

            // Convert boolean to string
            (FieldType::Str, Value::Bool(b)) => Value::String(b.to_string()),

            // Other conversions not supported
            _ => {
                return Err(ServerError::TypeMismatch {
                    field: field.name.clone(),
                    expected: field.field_type.as_str(),
                });
            }
        };

        // Now validate with the converted value
        for validator in &*field.validators {
            if let Err(e) = validator.validate(&converted_value) {
                return Err(ServerError::ValidationError {
                    field: field.name.clone(),
                    message: e.to_string(),
                });
            }
        }

        // Return success - the caller needs to update the original value
        // with the converted value if needed
        Ok(converted_value)
    }

    async fn process_json_body(request: Request<Body>) -> Result<Value, ServerError> {
        Json::<Value>::from_request(request, &())
            .await
            .map(|Json(body)| body)
            .map_err(ServerError::JsonParseError)
    }

    async fn process_form_body(request: Request<Body>) -> Result<Value, ServerError> {
        Form::<Value>::from_request(request, &())
            .await
            .map(|Form(body)| body)
            .map_err(ServerError::FormParseError)
    }

    fn get_request(&self) -> HashMap<&'static str, Value> {
        let uri = self.request.uri();
        [
            ("method", self.request.method().as_str().into()),
            if let Some(authority) = uri.authority() {
                ("authority", authority.as_str().into())
            } else {
                ("authority", serde_json::Value::Null)
            },
            if let Some(host) = uri.host() {
                ("host", host.into())
            } else {
                ("host", serde_json::Value::Null)
            },
            ("path", uri.path().into()),
            ("query", uri.query().into()),
            ("protocol", uri.scheme_str().into()),
            ("port", uri.port_u16().into()),
            // ("fragment", uri.fragment().into()),
        ]
        .into_iter()
        .collect()
    }

    fn get_header(&self) -> HashMap<String, Value> {
        self.request
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    Value::String(value.to_str().unwrap().to_owned()),
                )
            })
            .collect()
    }
}

impl Future for RequestProcessor {
    type Output = Result<Response, Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let config = Config::get();
        loop {
            match &mut self.state {
                ProcessingState::ValidatingAuth => {
                    if self.endpoint.annotation.is_jwt_auth() {
                        self.jwt_claim = {
                            let future = self
                                .request
                                .extract_parts::<TypedHeader<Authorization<Bearer>>>();
                            tokio::pin!(future);
                            match future.poll(cx) {
                                Poll::Pending => return Poll::Pending,
                                Poll::Ready(Ok(bearer)) => {
                                    let key =
                                        DecodingKey::from_secret(config.auth.jwt.secret.as_bytes());
                                    let validation = Validation::new(Algorithm::HS256);
                                    // Decode token
                                    let token_data: TokenData<Value> =
                                        decode(bearer.token(), &key, &validation).unwrap();
                                    Some(token_data.claims)
                                }
                                Poll::Ready(Err(e)) => {
                                    return Poll::Ready(Ok(ServerError::AuthenticationError {
                                        message: e.to_string(),
                                    }
                                    .into_response()));
                                }
                            }
                        };
                    } else {
                        // Baisc auth
                        // Extract the token from the authorization header
                        let future = self
                            .request
                            .extract_parts::<TypedHeader<Authorization<Basic>>>();
                        tokio::pin!(future);
                        match future.poll(cx) {
                            Poll::Pending => return Poll::Pending,
                            Poll::Ready(Ok(basic)) => {
                                if let Some(b) = config.auth.basic.as_ref() {
                                    if *b.username != basic.username()
                                        || *b.password != basic.password()
                                    {
                                        return Poll::Ready(Ok(ServerError::AuthenticationError {
                                            message: "Invalid username or password".to_string(),
                                        }
                                        .into_response()));
                                    }
                                } else {
                                    return Poll::Ready(Ok(ServerError::AuthenticationError {
                                        message: "Basic auth is not configured".to_string(),
                                    }
                                    .into_response()));
                                }
                            }
                            Poll::Ready(Err(e)) => {
                                return Poll::Ready(Ok(ServerError::AuthenticationError {
                                    message: e.to_string(),
                                }
                                .into_response()));
                            }
                        }
                    }
                    self.state = ProcessingState::ValidatingPath;
                }
                ProcessingState::ValidatingPath => {
                    let raw_path_params = {
                        // Extract path parameters using Axum's RawPathParams extractor
                        let future = self.request.extract_parts::<RawPathParams>();

                        tokio::pin!(future);
                        match future.poll(cx) {
                            Poll::Pending => return Poll::Pending,
                            Poll::Ready(Ok(params)) => params,
                            Poll::Ready(Err(e)) => {
                                return Poll::Ready(Ok(format!(
                                    "Failed to extract path parameters: {}",
                                    e
                                )
                                .into_response()));
                            }
                        }
                    };

                    // Process and validate each path parameter
                    for (param_name, param_value) in &raw_path_params {
                        // Find the corresponding path parameter field
                        if let Some(field) = self
                            .endpoint
                            .path_params
                            .iter()
                            .find(|f| f.name == param_name)
                        {
                            // Convert the value to the appropriate type based on the field definition
                            let value = match field.field_type {
                                FieldType::Str => Value::String(param_value.to_string()),
                                FieldType::Number => {
                                    if let Ok(num) = param_value.parse::<i64>() {
                                        Value::Number(num.into())
                                    } else if let Ok(float) = param_value.parse::<f64>() {
                                        match serde_json::Number::from_f64(float) {
                                            Some(n) => Value::Number(n),
                                            None => {
                                                return Poll::Ready(Ok(
                                                    format!("Invalid path parameter type for {}: could not convert to number", param_name)
                                                        .into_response()
                                                ));
                                            }
                                        }
                                    } else {
                                        return Poll::Ready(Ok(format!(
                                            "Invalid path parameter type for {}: expected a number",
                                            param_name
                                        )
                                        .into_response()));
                                    }
                                }
                                FieldType::Bool => match param_value.to_lowercase().as_str() {
                                    "true" => Value::Bool(true),
                                    "false" => Value::Bool(false),
                                    _ => {
                                        return Poll::Ready(Ok(
                                                format!("Invalid path parameter type for {}: expected a boolean", param_name)
                                                    .into_response()
                                            ));
                                    }
                                },
                                _ => {
                                    return Poll::Ready(Ok(format!(
                                        "Unsupported path parameter type for {}",
                                        param_name
                                    )
                                    .into_response()));
                                }
                            };

                            // Validate the value using our existing validation infrastructure
                            if let Err(e) = Self::validate_field(field, &value) {
                                return Poll::Ready(Ok(e.into_response()));
                            }

                            // Store the validated parameter
                            self.path_data.insert(param_name.to_string(), value);
                        }
                    }

                    // Check for missing required parameters
                    for field in &self.endpoint.path_params {
                        if !self.path_data.contains_key(&field.name) && field.required {
                            return Poll::Ready(Ok(
                                ServerError::MissingField(field.name.clone()).into_response()
                            ));
                        }
                    }

                    // Move to the next state
                    self.state = ProcessingState::ValidatingQuery;
                }
                ProcessingState::ValidatingQuery => {
                    let mut query = extract::Query::<Value>::try_from_uri(self.request.uri())
                        .map(|extract::Query(q)| q)
                        .unwrap_or_default();

                    let mut failed_validation = None;
                    for field in mem::take(&mut self.endpoint.query_params) {
                        if let Some(value) = query.get_mut(&field.name) {
                            match Self::validate_field(&field, value) {
                                Ok(converted_value) => *value = converted_value,
                                Err(e) => {
                                    failed_validation = Some(e);
                                    break;
                                }
                            }
                            self.query_data.insert(field.name.clone(), value.clone());
                        } else if let Some(default) = &field.default {
                            self.query_data.insert(field.name.clone(), default.clone());
                        } else if field.required {
                            failed_validation = Some(ServerError::MissingField(field.name.clone()));
                            break;
                        }
                    }

                    if let Some(error) = failed_validation {
                        return Poll::Ready(Ok(error.into_response()));
                    }

                    self.state = ProcessingState::ValidatingBody;
                }
                ProcessingState::ValidatingBody => {
                    let request_obj = self.get_request();
                    let header_obj = self.get_header();
                    if !self.endpoint.body_fields.is_empty() {
                        let request = mem::take(&mut self.request);
                        let body_fut: BoxFuture<Result<Value, ServerError>> =
                            match self.endpoint.body_type {
                                BodyKind::Json => Box::pin(Self::process_json_body(request)),
                                BodyKind::Form => Box::pin(Self::process_form_body(request)),
                            };

                        tokio::pin!(body_fut);

                        let body = match body_fut.poll(cx) {
                            Poll::Pending => return Poll::Pending,
                            Poll::Ready(Ok(value)) => value,
                            Poll::Ready(Err(e)) => {
                                return Poll::Ready(Ok(
                                    format!("Body parsing error: {:?}", e).into_response()
                                ));
                            }
                        };

                        let mut failed_validation = None;
                        for field in mem::take(&mut self.endpoint.body_fields) {
                            if let Some(value) = body.get(&field.name) {
                                if let Err(e) = Self::validate_field(&field, value) {
                                    failed_validation = Some(e);
                                    break;
                                }
                                self.body_data.insert(field.name.clone(), value.clone());
                            } else if let Some(default) = &field.default {
                                self.body_data.insert(field.name.clone(), default.clone());
                            } else if field.required {
                                failed_validation =
                                    Some(ServerError::MissingField(field.name.clone()));
                                break;
                            }
                        }

                        if let Some(error) = failed_validation {
                            return Poll::Ready(Ok(error.into_response()));
                        }
                    }

                    let script = mem::take(&mut self.endpoint.script);
                    let script = Box::leak(script.into_boxed_str());
                    let sso_fields = if let Some(provider) = self.endpoint.annotation.sso_provider {
                        match crate::config::get_sso_fields(provider) {
                            Some(fields) => Some(fields),
                            None => {
                                return Poll::Ready(Ok(format!(
                                    "Missing `[sso.{}]` config",
                                    provider.as_str()
                                )
                                .into_response()));
                            }
                        }
                    } else {
                        None
                    };
                    let path_data = mem::take(&mut self.path_data);
                    let query_data = mem::take(&mut self.query_data);
                    let body_data = mem::take(&mut self.body_data);
                    let pg_connection = self.endpoint.pg_connection.clone();
                    let sqlite_connection = self.endpoint.sqlite_connection.clone();
                    let redis_connection = self.endpoint.redis_connection.clone();
                    let handle: JoinHandle<Result<ReturnValue, VmError>> =
                        task::spawn_blocking(move || {
                            let mut vm =
                                Vm::new(pg_connection, sqlite_connection, redis_connection);
                            if let Some(fields) = sso_fields {
                                vm.inject_sso_instance(fields);
                            }
                            vm.register_extra_native_functions();
                            vm.compile(script)?;
                            vm.eval_function(
                                0,
                                &[
                                    Value::Object(path_data.into_iter().collect()),
                                    Value::Object(query_data.into_iter().collect()),
                                    Value::Object(body_data.into_iter().collect()),
                                    Value::Object(
                                        request_obj
                                            .into_iter()
                                            .map(|(k, v)| (k.to_owned(), v))
                                            .collect(),
                                    ),
                                    Value::Object(header_obj.into_iter().collect()),
                                ],
                            )
                        });
                    self.state = ProcessingState::Executing(handle);
                }
                ProcessingState::Executing(handle) => {
                    return match Pin::new(handle).poll(cx) {
                        Poll::Ready(Ok(result)) => match result {
                            Ok(value) => {
                                if let ReturnValue::Response(mut fields) = value {
                                    let mut response =
                                        Json(fields.remove("body").unwrap_or_default())
                                            .into_response();
                                    *response.status_mut() = StatusCode::from_u16(
                                        fields
                                            .remove("status_code")
                                            .map(|v| v.as_f64().unwrap())
                                            .unwrap_or(200f64)
                                            as u16,
                                    )
                                    .unwrap();
                                    if let Some(headers) = fields.remove("headers") {
                                        response.headers_mut().extend(
                                            headers.as_object().unwrap().into_iter().map(
                                                |(name, value)| {
                                                    (
                                                        HeaderName::try_from(name).unwrap(),
                                                        HeaderValue::from_str(
                                                            value.as_str().unwrap(),
                                                        )
                                                        .unwrap(),
                                                    )
                                                },
                                            ),
                                        );
                                    }

                                    Poll::Ready(Ok(response))
                                } else {
                                    Poll::Ready(Ok(Json(value).into_response()))
                                }
                            }
                            Err(VmError::CompileError) => {
                                Poll::Ready(Ok("Compile Error".into_response()))
                            }
                            Err(VmError::RuntimeError(err)) => {
                                Poll::Ready(Ok(format!("Runtime Error: {err}",).into_response()))
                            }
                        },
                        Poll::Ready(Err(err)) => {
                            Poll::Ready(Ok(format!("Error:: {err}").into_response()))
                        }
                        Poll::Pending => Poll::Pending,
                    };
                }
            }
        }
    }
}

impl Service<Request<Body>> for Endpoint {
    type Response = Response;
    type Error = Infallible;
    type Future = RequestProcessor;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        RequestProcessor::new(self.clone(), req)
    }
}

pub(crate) fn convert_field(field: ast::Field) -> Field {
    Field {
        name: field.name,
        field_type: field._type,
        required: field.required,
        default: field.default,
        validators: Arc::from(field.validators),
    }
}

pub fn render(template: &str, context: serde_json::Value) -> Result<String, String> {
    let engine = template::get_template_engine();
    engine.render(template, &context)
}