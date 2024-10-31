use aiscript_vm::{ReturnValue, Vm, VmError};
use axum::{
    body::Body,
    extract::{self, FromRequest, Request},
    response::{IntoResponse, Response},
    routing::*,
    Form, Json,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    convert::Infallible,
    fs,
    future::Future,
    mem,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use thiserror::Error;
use tokio::{
    net::TcpListener,
    task::{self, JoinHandle},
};
use tower::Service;

mod ast;
mod lexer;
mod parser;
mod validator;
use ast::*;
use validator::{convert_from_directive, Validator};

type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Field validation failed: {field}: {message}")]
    ValidationError { field: String, message: String },

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Type mismatch for field {field}: expected {expected}")]
    TypeMismatch {
        field: String,
        expected: &'static str,
    },

    #[error("Failed to parse JSON body: {0}")]
    JsonParseError(#[from] axum::extract::rejection::JsonRejection),

    #[error("Failed to parse Form body: {0}")]
    FormParseError(#[from] axum::extract::rejection::FormRejection),

    #[error("VM execution error: {0}")]
    VmError(#[from] VmError),

    #[error("Internal server error: {0}")]
    InternalError(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        (axum::http::StatusCode::BAD_REQUEST, self.to_string()).into_response()
    }
}

#[derive(Clone)]
struct Field {
    name: String,
    field_type: FieldType,
    required: bool,
    default: Option<Value>,
    validators: Arc<Vec<Box<dyn Validator>>>,
}

#[derive(Clone)]
struct Endpoint {
    query_params: Vec<Field>,
    body_type: BodyKind,
    body_fields: Vec<Field>,
    script: String,
    path_specs: Vec<PathSpec>,
}

enum ProcessingState {
    Init,
    ValidatingQuery,
    ValidatingBody,
    Executing(JoinHandle<Result<ReturnValue, VmError>>),
}

struct RequestProcessor {
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

pub async fn run(path: PathBuf, port: u16) {
    let route = parser::parse_route(&fs::read_to_string(path).unwrap()).unwrap();
    let mut router = Router::new();

    for route in [route] {
        let mut r = Router::new();
        for endpoint_spec in route.endpoints {
            let endpoint = Endpoint {
                query_params: endpoint_spec.query.into_iter().map(convert_field).collect(),
                body_type: endpoint_spec.body.kind,
                body_fields: endpoint_spec
                    .body
                    .fields
                    .into_iter()
                    .map(convert_field)
                    .collect(),
                script: endpoint_spec.statements,
                path_specs: endpoint_spec.path_specs,
            };

            for path_spec in &endpoint.path_specs[..endpoint.path_specs.len() - 1] {
                let service_fn = match path_spec.method {
                    HttpMethod::Get => get_service,
                    HttpMethod::Post => post_service,
                    HttpMethod::Put => put_service,
                    HttpMethod::Delete => delete_service,
                };
                r = r.route(&path_spec.path, service_fn(endpoint.clone()));
            }

            // avoid clone the last one
            let last_path_specs = &endpoint.path_specs[endpoint.path_specs.len() - 1];
            let service_fn = match last_path_specs.method {
                HttpMethod::Get => get_service,
                HttpMethod::Post => post_service,
                HttpMethod::Put => put_service,
                HttpMethod::Delete => delete_service,
            };
            r = r.route(&last_path_specs.path.clone(), service_fn(endpoint));
        }
        router = router.nest(&route.prefix, r);
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    axum::serve(TcpListener::bind(addr).await.unwrap(), router)
        .await
        .unwrap();
}

fn convert_field(field: ast::Field) -> Field {
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
