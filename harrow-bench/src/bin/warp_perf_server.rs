//! Warp performance benchmark server.
//!
//! Exposes the same routes as `axum-perf-server` for fair comparison.
//!
//! Routes:
//!   GET /text       -> "ok" (text/plain)
//!   GET /json/1kb   -> ~1KB JSON (10 user objects)
//!   GET /json/10kb  -> ~10KB JSON (100 user objects)
//!   GET /health     -> "ok" (text/plain)
//!
//! Usage: warp-perf-server [--bind ADDR] [--port PORT]

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const ALLOCATOR_NAME: &str = if cfg!(feature = "mimalloc") {
    "mimalloc"
} else {
    "system"
};

use std::net::SocketAddr;

use harrow_bench::{USERS_10, USERS_100};
use warp::Filter;

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
                eprintln!("usage: warp-perf-server [--bind ADDR] [--port PORT]");
                std::process::exit(1);
            }
        }
    }
    (bind, port)
}

#[tokio::main]
async fn main() {
    let (bind, port) = parse_args();
    let addr: SocketAddr = format!("{bind}:{port}").parse().unwrap();

    let text = warp::path!("text").and(warp::get()).map(|| "ok");

    let json_1kb = warp::path!("json" / "1kb")
        .and(warp::get())
        .map(|| warp::reply::json(&*USERS_10));

    let json_10kb = warp::path!("json" / "10kb")
        .and(warp::get())
        .map(|| warp::reply::json(&*USERS_100));

    let health = warp::path!("health").and(warp::get()).map(|| "ok");

    let routes = text.or(json_1kb).or(json_10kb).or(health);

    eprintln!("warp-perf-server listening on {addr} [allocator: {ALLOCATOR_NAME}]");
    warp::serve(routes).run(addr).await;
}
