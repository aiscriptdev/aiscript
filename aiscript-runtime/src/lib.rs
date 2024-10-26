#![allow(unused)]
use std::{
    collections::HashMap,
    convert::Infallible,
    future::Future,
    mem,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    extract::{self, FromRequest, Request},
    response::{IntoResponse, Response},
    routing::{delete_service, get, get_service, post_service, put_service},
    Form, Json, Router,
};
use lalrpop_util::lalrpop_mod;
use serde_json::Value;
use tokio::net::TcpListener;
use tokio::pin;
use tower::Service;

mod ast;
use ast::validators::*;
use ast::*;

lalrpop_mod!(
    #[rustfmt::skip]
    grammar
);

#[derive(Clone)]
struct EndpointImpl {
    query: Vec<Field>,
    body: Option<RequestBody>,
    handler: Handler,
}

impl EndpointImpl {
    fn execute(&mut self, req: Request) -> Result<Response, Infallible> {
        Ok(format!("Query: {:?}, Body: {:?}", self.query, self.body).into_response())
    }
}

struct Raw {
    query: Vec<Field>,
    body: Option<RequestBody>,
    handler: Handler,
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
                                for validator in &field.validators {
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
                    if let Some(mut raw_body) = mem::take(&mut self.raw.body) {
                        let body = match raw_body.kind {
                            BodyKind::Json => {
                                let fut = extract::Json::<Value>::from_request(
                                    mem::take(&mut self.req),
                                    &(),
                                );
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
                                let fut = extract::Form::<Value>::from_request(
                                    mem::take(&mut self.req),
                                    &(),
                                );
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

                        for field in &mut raw_body.fields {
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
                                    || (matches!(field._type, FieldType::Number)
                                        && value.is_number())
                                    || (matches!(field._type, FieldType::Bool)
                                        && value.is_boolean())
                                    || (matches!(field._type, FieldType::Array) && value.is_array())
                                {
                                    for validator in &field.validators {
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
                            }
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
                body: mem::take(&mut self.body),
                handler: mem::replace(&mut self.handler, Handler::Empty),
            },
            req,
            query: HashMap::new(),
            body: HashMap::new(),
            state: ExecuteFutureState::Pending,
        }
    }
}

pub async fn run(port: u16) {
    let mut router = Router::new().route("/", get(|| async { "Hello, World!" }));

    for route in Vec::<Route>::new() {
        let prefix = route.prefix;
        let mut r = Router::new();
        for endpoint in route.endpoints {
            let endpoint_impl = EndpointImpl {
                query: endpoint.query,
                body: endpoint.body,
                handler: endpoint.handler,
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

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_lalrpop() {
        let input = r#"
        get /a, put /a  {
            body {
                a: str
                b: bool
            }
        }
        "#;

        let ast = grammar::EndpointParser::new();
        let r = ast.parse(input).unwrap();
        println!("{:?}", r);
    }
}
