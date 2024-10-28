use std::{
    collections::HashMap,
    convert::Infallible,
    future::Future,
    mem,
    net::SocketAddr,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    extract::{self, FromRequest, Request},
    response::{IntoResponse, Response},
    routing::{delete_service, get, get_service, post_service, put_service},
    Form, Json, Router,
};
use parser::parse_route;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::pin;
use tower::Service;

mod ast;
mod lexer;
mod parser;
mod validator;
use ast::*;
use validator::{convert_from_directive, Validator};

#[derive(Clone)]
struct EndpointImpl {
    query: Vec<Field2>,
    body_kind: BodyKind,
    body: Vec<Field2>,
    statements: String,
}

#[derive(Clone)]
struct Field2 {
    name: String,
    _type: FieldType,
    required: bool,
    default: Option<Value>,
    validators: Arc<Vec<Box<dyn Validator>>>,
}

struct Raw {
    query: Vec<Field2>,
    body_kind: BodyKind,
    body: Vec<Field2>,
    statements: String,
}

struct ExecuteFuture {
    raw: Raw,
    req: Request,
    query: HashMap<String, Value>,
    body: HashMap<String, Value>,
    state: ExecuteFutureState,
}

enum ExecuteFutureState {
    Pending,
    Query,
    Body,
    Execute,
    Done,
}

impl Future for ExecuteFuture {
    type Output = Result<Response, Infallible>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.state {
                ExecuteFutureState::Pending => {
                    self.state = ExecuteFutureState::Query;
                }
                ExecuteFutureState::Query => {
                    let extract::Query(query) =
                        extract::Query::<Value>::try_from_uri(self.req.uri()).unwrap();
                    println!("Query: {:?}", query);
                    for mut field in mem::take(&mut self.raw.query) {
                        let field_value = query.get(&field.name);
                        if field.required && field_value.is_none() {
                            if let Some(default) = field.default.take() {
                                self.query.insert(field.name.clone(), default);
                            } else {
                                // Field is required but not found in query
                                return Poll::Ready(Ok(format!(
                                    "Field is required but not found in query: {}",
                                    field.name
                                )
                                .into_response()));
                            }
                        } else if let Some(value) = field_value {
                            if (matches!(field._type, FieldType::Str) && value.is_string())
                                || (matches!(field._type, FieldType::Number) && value.is_number())
                                || (matches!(field._type, FieldType::Bool) && value.is_boolean())
                                || (matches!(field._type, FieldType::Array) && value.is_array())
                            {
                                for validator in &*field.validators {
                                    if let Err(e) = validator.validate(value) {
                                        return Poll::Ready(Ok(format!(
                                            "Field validation failed: {}",
                                            e
                                        )
                                        .into_response()));
                                    }
                                }
                                self.query.insert(field.name.clone(), value.clone());
                            } else {
                                // Field type mismatch
                                return Poll::Ready(Ok(format!(
                                    "Field type mismatch: {}",
                                    field.name
                                )
                                .into_response()));
                            }
                        }
                    }
                    // let headers = self.req.headers();
                    self.state = ExecuteFutureState::Body;
                }
                ExecuteFutureState::Body => {
                    let mut raw_body = mem::take(&mut self.raw.body);
                    let body = match self.raw.body_kind {
                        BodyKind::Json => {
                            let fut =
                                extract::Json::<Value>::from_request(mem::take(&mut self.req), &());
                            pin!(fut);
                            let Json(body) = match fut.poll(cx) {
                                Poll::Pending => return Poll::Pending,
                                Poll::Ready(Ok(body)) => body,
                                Poll::Ready(Err(e)) => {
                                    return Poll::Ready(Ok(
                                        format!("Error: {:?}", e).into_response()
                                    ));
                                }
                            };
                            body
                        }
                        BodyKind::Form => {
                            let fut =
                                extract::Form::<Value>::from_request(mem::take(&mut self.req), &());
                            pin!(fut);
                            let Form(body) = match fut.poll(cx) {
                                Poll::Pending => return Poll::Pending,
                                Poll::Ready(Ok(body)) => body,
                                Poll::Ready(Err(e)) => {
                                    return Poll::Ready(Ok(
                                        format!("Error: {:?}", e).into_response()
                                    ));
                                }
                            };
                            body
                        }
                    };

                    for field in &mut raw_body {
                        let field_value = body.get(&field.name);
                        if field.required && field_value.is_none() {
                            if let Some(default) = field.default.take() {
                                self.body.insert(field.name.clone(), default);
                            } else {
                                return Poll::Ready(Ok(format!(
                                    "Field is required but not found in body: {}",
                                    field.name
                                )
                                .into_response()));
                            }
                        } else if let Some(value) = field_value {
                            if (matches!(field._type, FieldType::Str) && value.is_string())
                                || (matches!(field._type, FieldType::Number) && value.is_number())
                                || (matches!(field._type, FieldType::Bool) && value.is_boolean())
                                || (matches!(field._type, FieldType::Array) && value.is_array())
                            {
                                for validator in &*field.validators {
                                    if let Err(e) = validator.validate(value) {
                                        return Poll::Ready(Ok(format!(
                                            "Field validation failed: {}",
                                            e
                                        )
                                        .into_response()));
                                    }
                                }
                                self.body.insert(field.name.clone(), value.clone());
                            } else {
                                return Poll::Ready(Ok(format!(
                                    "Field type mismatch: {}",
                                    field.name
                                )
                                .into_response()));
                            }
                            // }
                        }
                    }

                    self.state = ExecuteFutureState::Execute;
                }
                ExecuteFutureState::Execute => {
                    self.state = ExecuteFutureState::Done;
                }
                ExecuteFutureState::Done => {
                    break;
                }
            }
        }

        Poll::Ready(Ok(format!(
            "Query: {:?}, Body: {:?}",
            self.query, self.body
        )
        .into_response()))
    }
}

impl Service<Request> for EndpointImpl {
    type Response = Response;

    type Error = Infallible;

    // type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    type Future = ExecuteFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        // Box::pin(self)
        // extract::Query::from_request(req).await;
        // self.clone()
        ExecuteFuture {
            raw: Raw {
                query: mem::take(&mut self.query),
                body_kind: mem::take(&mut self.body_kind),
                body: mem::take(&mut self.body),
                statements: mem::take(&mut self.statements),
            },
            req,
            query: HashMap::new(),
            body: HashMap::new(),
            state: ExecuteFutureState::Pending,
        }
    }
}

pub async fn run(path: PathBuf, port: u16) {
    let input = std::fs::read_to_string(path).unwrap();
    let route = parse_route(&input).unwrap();
    let mut router = Router::new().route("/", get(|| async { "Hello, World!" }));

    for route in [route] {
        let prefix = route.prefix;
        let mut r = Router::new();
        for endpoint in route.endpoints {
            let endpoint_impl = EndpointImpl {
                query: endpoint
                    .query
                    .into_iter()
                    .map(convert_field_to_field2)
                    .collect(),
                body_kind: endpoint.body.kind,
                body: endpoint
                    .body
                    .fields
                    .into_iter()
                    .map(convert_field_to_field2)
                    .collect(),
                statements: endpoint.statements,
            };
            for path_spec in endpoint.path_specs {
                let service_fn = match path_spec.method {
                    HttpMethod::Get => get_service,
                    HttpMethod::Post => post_service,
                    HttpMethod::Put => put_service,
                    HttpMethod::Delete => delete_service,
                };
                r = r.route(&path_spec.path, service_fn(endpoint_impl.clone()));
            }
        }
        router = router.nest(&prefix, r);
    }

    println!("{:?}", router);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

fn convert_field_to_field2(field: Field) -> Field2 {
    let validators = field
        .directives
        .into_iter()
        .map(|v| convert_from_directive(v))
        .collect();
    Field2 {
        name: field.name,
        _type: field._type,
        required: field.required,
        default: field.default,
        validators: Arc::new(validators),
    }
}
