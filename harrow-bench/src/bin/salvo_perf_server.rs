//! Salvo performance benchmark server.
//!
//! Exposes the same routes as `axum-perf-server` for fair comparison.
//!
//! Routes:
//!   GET /text       -> "ok" (text/plain)
//!   GET /json/1kb   -> ~1KB JSON (10 user objects)
//!   GET /json/10kb  -> ~10KB JSON (100 user objects)
//!   GET /health     -> "ok" (text/plain)
//!
//! Usage: salvo-perf-server [--bind ADDR] [--port PORT]

harrow_bench::setup_allocator!();

use harrow_bench::{USERS_10, USERS_100, User};
use salvo::prelude::*;

#[handler]
async fn text_handler() -> &'static str {
    "ok"
}

#[handler]
async fn json_1kb_handler() -> Json<&'static Vec<User>> {
    Json(&*USERS_10)
}

#[handler]
async fn json_10kb_handler() -> Json<&'static Vec<User>> {
    Json(&*USERS_100)
}

#[handler]
async fn health_handler() -> &'static str {
    "ok"
}

fn parse_args() -> (String, u16) {
    let args: Vec<String> = std::env::args().collect();
    let mut bind = "127.0.0.1".to_string();
    let mut port: u16 = 3090;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--bind" => {
                bind = args.get(i + 1).expect("--bind requires an address").clone();
                i += 2;
            }
            "--port" => {
                port = args
                    .get(i + 1)
                    .expect("--port requires a number")
                    .parse()
                    .expect("invalid port number");
                i += 2;
            }
            other => {
                eprintln!("unknown option: {other}");
                eprintln!("usage: salvo-perf-server [--bind ADDR] [--port PORT]");
                std::process::exit(1);
            }
        }
    }
    (bind, port)
}

#[tokio::main]
async fn main() {
    let (bind, port) = parse_args();
    let addr: std::net::SocketAddr = format!("{bind}:{port}").parse().unwrap();

    let router = Router::new()
        .push(Router::with_path("text").get(text_handler))
        .push(
            Router::with_path("json")
                .push(Router::with_path("1kb").get(json_1kb_handler))
                .push(Router::with_path("10kb").get(json_10kb_handler)),
        )
        .push(Router::with_path("health").get(health_handler));

    let acceptor = TcpListener::new(addr).bind().await;
    eprintln!("salvo-perf-server listening on {addr} [allocator: {ALLOCATOR_NAME}]");
    Server::new(acceptor).serve(router).await;
}
