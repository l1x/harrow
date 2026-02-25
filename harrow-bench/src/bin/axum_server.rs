//! Minimal Axum server for framework comparison benchmarks.
//!
//! Identical endpoints to harrow_server — raw framework overhead only.
//! Usage: axum-server [--port PORT]

use axum::{Router, extract::Path, routing::get, Json};
use serde_json::{Value, json};

async fn hello() -> &'static str {
    "hello, world"
}

async fn greet(Path(name): Path<String>) -> String {
    format!("hello, {name}")
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

fn parse_port() -> u16 {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--port" {
            if let Some(p) = args.get(i + 1) {
                return p.parse().expect("invalid port number");
            }
        }
    }
    3000
}

#[tokio::main]
async fn main() {
    let port = parse_port();
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    let app = Router::new()
        .route("/", get(hello))
        .route("/greet/{name}", get(greet))
        .route("/health", get(health));

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    eprintln!("axum listening on {addr}");
    axum::serve(listener, app).await.unwrap();
}
