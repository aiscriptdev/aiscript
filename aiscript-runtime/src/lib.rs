use ast::HttpMethod;
use axum::{response::Html, routing::*};
use std::{fs, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use walkdir::WalkDir;

use crate::endpoint::{convert_field, Endpoint};
mod ast;
mod endpoint;
mod error;
mod lexer;
mod openapi;
mod parser;
mod validator;

fn read_routes() -> Vec<ast::Route> {
    let mut routes = Vec::new();
    for entry in WalkDir::new("routes")
        .contents_first(true)
        .into_iter()
        .filter_entry(|e| {
            e.file_type().is_file()
                && e.file_name()
                    .to_str()
                    .map(|s| s.ends_with(".ai"))
                    .unwrap_or(false)
        })
        .filter_map(|e| e.ok())
    {
        let file_path = entry.path();
        let input = fs::read_to_string(file_path).unwrap();
        let route = parser::parse_route(&input).unwrap();
        routes.push(route);
    }
    routes
}

pub async fn run(path: Option<PathBuf>, port: u16) {
    let routes = if let Some(path) = path {
        vec![parser::parse_route(&fs::read_to_string(path).unwrap()).unwrap()]
    } else {
        read_routes()
    };
    let mut router = Router::new();

    let openapi = serde_json::to_string(&openapi::OpenAPIGenerator::generate(&routes)).unwrap();
    router = router
        .route("/openapi.json", get(move || async { openapi }))
        .route(
            "/redoc",
            get(|| async { Html(include_str!("openapi/redoc.html")) }),
        );
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
