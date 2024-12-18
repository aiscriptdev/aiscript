use ast::HttpMethod;
use axum::{response::Html, routing::*};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::{fs, net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use walkdir::WalkDir;

use crate::endpoint::{convert_field, Endpoint};
mod ast;
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
    match std::env::var("DATABASE_URL") {
        Ok(url) => PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await
            .ok(),
        Err(_) => None,
    }
}

async fn run_server(
    path: Option<PathBuf>,
    port: u16,
    reload_rx: Option<broadcast::Receiver<ReloadSignal>>,
) {
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
    let openapi = serde_json::to_string(&openapi::OpenAPIGenerator::generate(&routes)).unwrap();
    router = router
        .route("/openapi.json", get(move || async { openapi }))
        .route(
            "/redoc",
            get(|| async { Html(include_str!("openapi/redoc.html")) }),
        );

    let pg_connection = get_pg_connection().await;
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
                pg_connection: pg_connection.as_ref().cloned(),
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
