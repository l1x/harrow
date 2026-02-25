//! Minimal Harrow server for framework comparison benchmarks.
//!
//! No o11y, no timeout middleware — raw framework overhead only.
//! Usage: harrow-server [--port PORT]

use harrow::{App, Request, Response};

async fn hello(_req: Request) -> Response {
    Response::text("hello, world")
}

async fn greet(req: Request) -> Response {
    let name = req.param("name");
    Response::text(format!("hello, {name}"))
}

async fn health(_req: Request) -> Response {
    Response::json(&serde_json::json!({ "status": "ok" }))
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

    let app = App::new()
        .get("/", hello)
        .get("/greet/:name", greet)
        .get("/health", health);

    eprintln!("harrow listening on {addr}");
    harrow::serve(app, addr).await.unwrap();
}
