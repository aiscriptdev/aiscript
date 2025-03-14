use ast::HttpMethod;
use axum::Json;
use axum::response::IntoResponse;
use axum::{response::Html, routing::*};
use hyper::StatusCode;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{PgPool, SqlitePool};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::{fs, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use walkdir::WalkDir;

use crate::endpoint::{Endpoint, convert_field};
pub use config::Config;
mod ast;
mod config;
mod endpoint;
mod error;
mod openapi;
mod parser;
mod utils;

use aiscript_lexer as lexer;

#[derive(Debug, Clone)]
struct ReloadSignal;

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
        if let Some(route) = read_single_route(file_path) {
            routes.push(route);
        }
    }
    routes
}

fn read_single_route(file_path: &Path) -> Option<ast::Route> {
    match fs::read_to_string(file_path) {
        Ok(input) => match parser::parse_route(&input) {
            Ok(route) => return Some(route),
            Err(e) => eprintln!("Error parsing route file {:?}: {}", file_path, e),
        },
        Err(e) => eprintln!("Error reading route file {:?}: {}", file_path, e),
    }

    None
}

pub async fn run(path: Option<PathBuf>, port: u16, reload: bool) {
    if !reload {
        // Run without reload functionality
        run_server(path, port, None).await;
        return;
    }

    // Create a channel for reload coordination
    let (tx, _) = broadcast::channel::<ReloadSignal>(1);
    let tx = Arc::new(tx);

    // Set up file watcher
    let watcher_tx = tx.clone();
    let mut watcher = setup_watcher(move |event| {
        // Only trigger reload for .ai files
        if let Some(path) = event.paths.first().and_then(|p| p.to_str()) {
            if path.ends_with(".ai") {
                watcher_tx.send(ReloadSignal).unwrap();
            }
        }
    })
    .expect("Failed to setup watcher");

    // Watch the routes directory
    watcher
        .watch(Path::new("routes"), RecursiveMode::Recursive)
        .expect("Failed to watch routes directory");

    loop {
        let mut rx = tx.subscribe();
        let server_handle = tokio::spawn(run_server(path.clone(), port, Some(rx.resubscribe())));

        // Wait for reload signal
        match rx.recv().await {
            Ok(_) => {
                println!("📑 Routes changed, reloading server...");
                // Give some time for pending requests to complete
                tokio::time::sleep(Duration::from_millis(100)).await;
                server_handle.abort();
            }
            Err(_) => {
                break;
            }
        }
    }
}

fn setup_watcher<F>(mut callback: F) -> notify::Result<RecommendedWatcher>
where
    F: FnMut(notify::Event) + Send + 'static,
{
    let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        match res {
            Ok(event) => {
                // Only trigger on write/create/remove events
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    callback(event);
                }
            }
            Err(e) => println!("Watch error: {:?}", e),
        }
    })?;
    Ok(watcher)
}

pub async fn get_pg_connection() -> Option<PgPool> {
    let config = Config::get();
    match config.database.get_postgres_url() {
        Some(url) => PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .ok(),
        None => None,
    }
}

pub async fn get_sqlite_connection() -> Option<SqlitePool> {
    let config = Config::get();
    match config.database.get_sqlite_url() {
        Some(url) => SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .ok(),
        None => None,
    }
}

pub async fn get_redis_connection() -> Option<redis::aio::MultiplexedConnection> {
    let config = Config::get();
    match config.database.get_redis_url() {
        Some(url) => {
            let client = redis::Client::open(url).unwrap();
            let conn = client.get_multiplexed_async_connection().await.unwrap();
            Some(conn)
        }
        None => None,
    }
}

async fn run_server(
    path: Option<PathBuf>,
    port: u16,
    reload_rx: Option<broadcast::Receiver<ReloadSignal>>,
) {
    let config = Config::get();

    let routes = if let Some(file_path) = path {
        read_single_route(&file_path).into_iter().collect()
    } else {
        read_routes()
    };

    if routes.is_empty() {
        eprintln!("Warning: No valid routes found!");
        return;
    }

    let mut router = Router::new();
    let openapi = openapi::OpenAPIGenerator::generate(&routes);
    router = router.route("/openapi.json", get(move || async { Json(openapi) }));

    if config.apidoc.enabled {
        match config.apidoc.doc_type {
            config::ApiDocType::Swagger => {
                router = router.route(
                    &config.apidoc.path,
                    get(move || async { Html(include_str!("openapi/swagger.html")) }),
                );
            }
            config::ApiDocType::Redoc => {
                router = router.route(
                    &config.apidoc.path,
                    get(|| async { Html(include_str!("openapi/redoc.html")) }),
                );
            }
        }
    }

    let pg_connection = get_pg_connection().await;
    let sqlite_connection = get_sqlite_connection().await;
    let redis_connection = get_redis_connection().await;
    for route in routes {
        let mut r = Router::new();
        for endpoint_spec in route.endpoints {
            let endpoint = Endpoint {
                annotation: endpoint_spec.annotation.or(&route.annotation),
                path_params: endpoint_spec.path.into_iter().map(convert_field).collect(),
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
                pg_connection: pg_connection.as_ref().cloned(),
                sqlite_connection: sqlite_connection.as_ref().cloned(),
                redis_connection: redis_connection.as_ref().cloned(),
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

        if route.prefix == "/" {
            // axum don't allow use nest() with root path
            router = router.merge(r);
        } else {
            router = router.nest(&route.prefix, r);
        }
    }

    // Add a custom 404 handler for unmatched routes
    async fn handle_404() -> impl IntoResponse {
        let error_json = serde_json::json!({
            "message": "Not Found"
        });

        (StatusCode::NOT_FOUND, Json(error_json))
    }

    // Add the fallback handler to the router
    router = router.fallback(handle_404);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.unwrap();

    match reload_rx {
        Some(mut rx) => {
            // Create a shutdown signal for reload case
            let (close_tx, close_rx) = tokio::sync::oneshot::channel();

            // Handle reload messages
            let shutdown_task = tokio::spawn(async move {
                if rx.recv().await.is_ok() {
                    close_tx.send(()).unwrap();
                }
            });

            // Run the server with graceful shutdown
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = close_rx.await;
                })
                .await
                .unwrap();

            shutdown_task.abort();
        }
        None => {
            // Run without reload capability
            axum::serve(listener, router).await.unwrap();
        }
    }
}

fn generate_openapi_json(routes: &[ast::Route]) -> serde_json::Value {
    let mut openapi = serde_json::json!({
        "openapi": "3.0.0",
        "info": {
            "title": "AIScript API",
            "version": "1.0.0",
            "description": "API documentation for AIScript"
        },
        "paths": {},
    });

    //Add paths from routes
    let paths = openapi["paths"].as_object_mut().unwrap();

    for route in routes {
        for endpoint in &route.endpoints {
            for path_spec in &endpoint.path_specs {
                let path = if route.prefix == "/" {
                    path_spec.path.clone()
                } else {
                    format!("{}{}", route.prefix, path_spec.path)
                };

                let method = match path_spec.method {
                    ast::HttpMethod::Get => "get",
                    ast::HttpMethod::Post => "post",
                    ast::HttpMethod::Put => "put",
                    ast::HttpMethod::Delete => "delete",
                };

                //For each method, add the path and method to the paths object
                if !paths.contains_key(&path) {
                    paths.insert(path.clone(), serde_json::json!({}));
                }

                //Add the method to the path
                let path_obj = paths.get_mut(&path).unwrap();
                path_obj[method] = serde_json::json!({
                    "summary": format!("{} {}", method.to_uppercase(), path),
                    "responses": {
                        "200": {
                            "description": "Successful response"
                        }
                    }
                });
            }
        }
    }

    openapi
}
