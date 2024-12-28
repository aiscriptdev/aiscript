use aiscript_directive::Validator;
use aiscript_vm::{ReturnValue, Vm, VmError};
use axum::{
    body::Body,
    extract::{self, FromRequest, Request},
    response::{IntoResponse, Response},
    Form, Json, RequestExt,
};
use axum_extra::{
    headers::{
        authorization::{Basic, Bearer},
        Authorization,
    },
    TypedHeader,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, TokenData, Validation};
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
    ast::{self, *},
    Config,
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
    pub auth: Auth,
    pub query_params: Vec<Field>,
    pub body_type: BodyKind,
    pub body_fields: Vec<Field>,
    pub script: String,
    pub path_specs: Vec<PathSpec>,
    pub pg_connection: Option<PgPool>,
    pub sqlite_connection: Option<SqlitePool>,
    pub redis_connection: Option<redis::aio::MultiplexedConnection>,
}

enum ProcessingState {
    ValidatingAuth,
    ValidatingQuery,
    ValidatingBody,
    Executing(JoinHandle<Result<ReturnValue, VmError>>),
}

pub struct RequestProcessor {
    endpoint: Endpoint,
    request: Request<Body>,
    jwt_claim: Option<Value>,
    query_data: HashMap<String, Value>,
    body_data: HashMap<String, Value>,
    state: ProcessingState,
}

impl RequestProcessor {
    fn new(endpoint: Endpoint, request: Request<Body>) -> Self {
        let state = if endpoint.auth.is_required() {
            ProcessingState::ValidatingAuth
        } else {
            ProcessingState::ValidatingQuery
        };
        Self {
            endpoint,
            request,
            jwt_claim: None,
            query_data: HashMap::new(),
            body_data: HashMap::new(),
            state,
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
                    if matches!(self.endpoint.auth, Auth::Jwt) {
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
                                    .into_response()))
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
                                .into_response()))
                            }
                        }
                    }
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
                    let query_data = mem::take(&mut self.query_data);
                    let body_data = mem::take(&mut self.body_data);
                    let pg_connection = self.endpoint.pg_connection.clone();
                    let sqlite_connection = self.endpoint.sqlite_connection.clone();
                    let redis_connection = self.endpoint.redis_connection.clone();
                    let handle: JoinHandle<Result<ReturnValue, VmError>> =
                        task::spawn_blocking(move || {
                            let mut vm =
                                Vm::new(pg_connection, sqlite_connection, redis_connection);
                            vm.compile(script)?;
                            vm.eval_function(
                                0,
                                &[
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
                            Ok(value) => Poll::Ready(Ok(Json(value).into_response())),
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
        validators: Arc::new(field.validators),
    }
}
