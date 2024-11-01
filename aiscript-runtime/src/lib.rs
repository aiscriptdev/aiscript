use ast::HttpMethod;
use axum::routing::*;
use std::{fs, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;

use crate::endpoint::{convert_field, Endpoint};
mod ast;
mod endpoint;
mod error;
mod lexer;
mod openapi;
mod parser;
mod validator;

pub async fn run(path: PathBuf, port: u16) {
    let route = parser::parse_route(&fs::read_to_string(path).unwrap()).unwrap();
    let mut router = Router::new();

    let routes = [route];
    let openapi = serde_json::to_string(&openapi::OpenAPIGenerator::generate(&routes)).unwrap();
    router = router.route("/openapi.json", get(move || async { openapi }));
    for route in routes {
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
