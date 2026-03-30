//! Comprehensive test server for Vegeta load testing (Monoio/io_uring backend).
//!
//! This server exposes Harrow features for load testing with the io_uring backend:
//! - Basic routes (GET, POST, PUT, DELETE)
//! - Path parameters
//! - JSON/text responses
//! - Health/liveness/readiness probes
//! - Error responses (404, 405)
//!
//! Note: Some middleware (timeout, request-id, CORS) is not yet available for monoio.
//!
//! Run with: cargo run --example vegeta_target_monoio --features monoio,json --no-default-features

mod common;

use harrow::runtime::monoio::run;
use harrow::{App, Request, Response};

async fn root(_req: Request) -> Response {
    Response::text("hello from io_uring!")
}

async fn health(_req: Request) -> Response {
    Response::json(&serde_json::json!({
        "status": "ok",
        "backend": "monoio/io_uring",
    }))
}

async fn echo(req: Request) -> Response {
    match req.body_json::<serde_json::Value>().await {
        Ok(body) => Response::json(&body),
        Err(_) => Response::text("invalid json").status(400),
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    let (bind, port) = common::parse_args("vegeta_target_monoio");
    let addr: std::net::SocketAddr = format!("{bind}:{port}").parse().unwrap();

    let app = App::new()
        .not_found_handler(common::not_found_handler)
        .health_handler("/health", health)
        .liveness_handler("/live", common::liveness)
        .readiness_handler("/ready", common::readiness)
        .get("/", root)
        .get("/users/:id", common::get_user)
        .post("/users", common::create_user)
        .get("/users/:user_id/posts/:post_id", common::get_user_posts)
        .post("/echo", echo)
        .put("/echo", echo)
        .delete("/echo", |_req| async move { Response::text("deleted") })
        .get("/cpu", common::cpu_intensive);

    tracing::info!("Monoio/io_uring server starting on http://{}", addr);

    if let Err(e) = run(app, addr) {
        tracing::error!("server error: {}", e);
        std::process::exit(1);
    }
}
