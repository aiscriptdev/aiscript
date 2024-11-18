use aiscript_vm::{ReturnValue, Vm, VmError};
use axum::{
    body::Body,
    extract::{self, FromRequest, Request},
    response::{IntoResponse, Response},
    Form, Json,
};
use serde_json::Value;
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
    ast::{self, *},
    validator::{convert_from_directive, Validator},
};

use crate::error::ServerError;

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        (axum::http::StatusCode::BAD_REQUEST, self.to_string()).into_response()
    }
}

#[derive(Clone)]
pub struct Field {
    name: String,
    field_type: FieldType,
    required: bool,
    default: Option<Value>,
    validators: Arc<Vec<Box<dyn Validator>>>,
}

#[derive(Clone)]
pub struct Endpoint {
    pub query_params: Vec<Field>,
    pub body_type: BodyKind,
    pub body_fields: Vec<Field>,
    pub script: String,
    pub path_specs: Vec<PathSpec>,
}

enum ProcessingState {
    Init,
    ValidatingQuery,
    ValidatingBody,
    Executing(JoinHandle<Result<ReturnValue, VmError>>),
}

pub struct RequestProcessor {
    endpoint: Endpoint,
    request: Request<Body>,
    query_data: HashMap<String, Value>,
    body_data: HashMap<String, Value>,
    state: ProcessingState,
}

impl RequestProcessor {
    fn new(endpoint: Endpoint, request: Request<Body>) -> Self {
        Self {
            endpoint,
            request,
            query_data: HashMap::new(),
            body_data: HashMap::new(),
            state: ProcessingState::Init,
        }
    }
    fn validate_field(field: &Field, value: &Value) -> Result<(), ServerError> {
        let type_valid = matches!(
            (field.field_type, value),
            (FieldType::Str, Value::String(_))
                | (FieldType::Number, Value::Number(_))
                | (FieldType::Bool, Value::Bool(_))
                | (FieldType::Array, Value::Array(_))
        );

        if !type_valid {
            return Err(ServerError::TypeMismatch {
                field: field.name.clone(),
                expected: field.field_type.as_str(),
            });
        }

        for validator in &*field.validators {
            if let Err(e) = validator.validate(value) {
                return Err(ServerError::ValidationError {
                    field: field.name.clone(),
                    message: e.to_string(),
                });
            }
        }
        Ok(())
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

    fn request_instance(&self) -> HashMap<&'static str, Value> {
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

    fn header_instance(&self) -> HashMap<String, Value> {
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
        loop {
            match &mut self.state {
                ProcessingState::Init => {
                    self.state = ProcessingState::ValidatingQuery;
                }
                ProcessingState::ValidatingQuery => {
                    let query = extract::Query::<Value>::try_from_uri(self.request.uri())
                        .map(|extract::Query(q)| q)
                        .unwrap_or_default();

                    let mut failed_validation = None;
                    for field in mem::take(&mut self.endpoint.query_params) {
                        if let Some(value) = query.get(&field.name) {
                            if let Err(e) = Self::validate_field(&field, value) {
                                failed_validation = Some(e);
                                break;
                            }
                            self.query_data.insert(field.name.clone(), value.clone());
                        } else if field.required {
                            if let Some(default) = &field.default {
                                self.query_data.insert(field.name.clone(), default.clone());
                            } else {
                                failed_validation =
                                    Some(ServerError::MissingField(field.name.clone()));
                                break;
                            }
                        }
                    }

                    if let Some(error) = failed_validation {
                        return Poll::Ready(Ok(error.into_response()));
                    }

                    self.state = ProcessingState::ValidatingBody;
                }
                ProcessingState::ValidatingBody => {
                    let request_instance = self.request_instance();
                    let header_instance = self.header_instance();
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
                            ))
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
                        } else if field.required {
                            if let Some(default) = &field.default {
                                self.body_data.insert(field.name.clone(), default.clone());
                            } else {
                                failed_validation =
                                    Some(ServerError::MissingField(field.name.clone()));
                                break;
                            }
                        }
                    }

                    if let Some(error) = failed_validation {
                        return Poll::Ready(Ok(error.into_response()));
                    }

                    let script = mem::take(&mut self.endpoint.script);
                    let script = Box::leak(script.into_boxed_str());
                    let query_data = mem::take(&mut self.query_data);
                    let body_data = mem::take(&mut self.body_data);
                    let handle = task::spawn_blocking(move || {
                        // aiscript_vm::eval(script)
                        let mut vm = Vm::new();
                        vm.compile(script)?;
                        vm.inject_variables(query_data);
                        vm.inject_variables(body_data);
                        vm.inject_instance("request", request_instance);
                        vm.inject_instance("header", header_instance);
                        vm.interpret()
                    });
                    self.state = ProcessingState::Executing(handle);
                }
                ProcessingState::Executing(handle) => {
                    return match Pin::new(handle).poll(cx) {
                        Poll::Ready(result) => Poll::Ready(Ok(format!(
                            "Query: {:?}, Body: {:?}, Result: {:?}",
                            self.query_data, self.body_data, result
                        )
                        .into_response())),
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
        validators: Arc::new(
            field
                .directives
                .into_iter()
                .map(convert_from_directive)
                .collect(),
        ),
    }
}
