use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use harrow_core::middleware::Next;
use harrow_core::request::Request;
use harrow_core::response::Response;
use harrow_core::route::App;

/// Shared counter used as application state.
struct HitCounter(AtomicUsize);

// -- Handlers ----------------------------------------------------------------

async fn hello(_req: Request) -> Response {
    Response::text("hello")
}

async fn greet(req: Request) -> Response {
    let name = req.param("name");
    Response::text(format!("hello, {name}"))
}

async fn state_handler(req: Request) -> Response {
    let counter = req.state::<Arc<HitCounter>>();
    let count = counter.0.fetch_add(1, Ordering::Relaxed) + 1;
    Response::text(format!("hits: {count}"))
}

// -- Middleware --------------------------------------------------------------

/// Prepends "before|" to the response body and appends "|after".
async fn wrap_middleware(req: Request, next: Next) -> Response {
    let resp = next.run(req).await;
    // We can't easily read the body back, so add a header to prove we ran.
    resp.header("x-wrap", "true")
}

/// A second middleware that adds its own header.
async fn second_middleware(req: Request, next: Next) -> Response {
    let resp = next.run(req).await;
    resp.header("x-second", "yes")
}

// -- Helpers -----------------------------------------------------------------

/// Spin up the server on a random port, return the bound address.
async fn start_server(app: App) -> SocketAddr {
    // Bind to port 0 to get an OS-assigned free port, then drop the listener
    // so serve_with_shutdown can rebind it.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        harrow_server::serve_with_shutdown(app, addr, async {
            let _ = rx.await;
        })
        .await
        .unwrap();
    });

    // Give the server a moment to bind.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Keep the shutdown sender alive by leaking it (test cleanup is fine).
    std::mem::forget(tx);

    addr
}

/// Simple HTTP/1.1 GET via raw TCP, returns (status, headers, body).
async fn http_get(addr: SocketAddr, path: &str) -> (u16, Vec<(String, String)>, String) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let req = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await.unwrap();

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    let raw = String::from_utf8_lossy(&buf);

    let mut parts = raw.splitn(2, "\r\n\r\n");
    let head = parts.next().unwrap_or("");
    let body = parts.next().unwrap_or("").to_string();

    let mut lines = head.lines();
    let status_line = lines.next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);

    let headers: Vec<(String, String)> = lines
        .filter_map(|line| {
            let mut parts = line.splitn(2, ": ");
            let key = parts.next()?.to_lowercase();
            let val = parts.next()?.to_string();
            Some((key, val))
        })
        .collect();

    // Handle chunked transfer encoding: extract the actual body.
    let body = if headers.iter().any(|(k, v)| k == "transfer-encoding" && v.contains("chunked")) {
        decode_chunked(&body)
    } else {
        body
    };

    (status, headers, body)
}

fn decode_chunked(raw: &str) -> String {
    let mut result = String::new();
    let mut remaining = raw;
    loop {
        let (size_str, rest) = remaining.split_once("\r\n").unwrap_or(("0", ""));
        let size = usize::from_str_radix(size_str.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        result.push_str(&rest[..size]);
        // Skip past the chunk data and the trailing \r\n.
        remaining = &rest[size..];
        if remaining.starts_with("\r\n") {
            remaining = &remaining[2..];
        }
    }
    result
}

fn header_val<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

// -- Tests -------------------------------------------------------------------

#[tokio::test]
async fn basic_routing() {
    let app = App::new()
        .get("/hello", hello)
        .get("/greet/:name", greet);

    let addr = start_server(app).await;

    let (status, _, body) = http_get(addr, "/hello").await;
    assert_eq!(status, 200);
    assert_eq!(body, "hello");

    let (status, _, body) = http_get(addr, "/greet/world").await;
    assert_eq!(status, 200);
    assert_eq!(body, "hello, world");
}

#[tokio::test]
async fn returns_404_for_unknown_path() {
    let app = App::new().get("/hello", hello);
    let addr = start_server(app).await;

    let (status, _, _) = http_get(addr, "/nope").await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn returns_405_for_wrong_method() {
    let app = App::new().post("/hello", hello);
    let addr = start_server(app).await;

    // GET against a POST-only route.
    let (status, _, _) = http_get(addr, "/hello").await;
    assert_eq!(status, 405);
}

#[tokio::test]
async fn middleware_runs_in_order() {
    let app = App::new()
        .middleware(wrap_middleware)
        .middleware(second_middleware)
        .get("/hello", hello);

    let addr = start_server(app).await;

    let (status, headers, body) = http_get(addr, "/hello").await;
    assert_eq!(status, 200);
    assert_eq!(body, "hello");

    // wrap_middleware runs first, sees response on the way back -> sets x-wrap
    assert_eq!(header_val(&headers, "x-wrap"), Some("true"));
    // second_middleware runs second, sets x-second
    assert_eq!(header_val(&headers, "x-second"), Some("yes"));
}

#[tokio::test]
async fn state_injection_works() {
    let counter = Arc::new(HitCounter(AtomicUsize::new(0)));

    let app = App::new()
        .state(counter.clone())
        .get("/count", state_handler);

    let addr = start_server(app).await;

    let (_, _, body) = http_get(addr, "/count").await;
    assert_eq!(body, "hits: 1");

    let (_, _, body) = http_get(addr, "/count").await;
    assert_eq!(body, "hits: 2");

    let (_, _, body) = http_get(addr, "/count").await;
    assert_eq!(body, "hits: 3");

    // Also verify from the original handle.
    assert_eq!(counter.0.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn middleware_and_state_together() {
    let counter = Arc::new(HitCounter(AtomicUsize::new(0)));

    let app = App::new()
        .state(counter.clone())
        .middleware(wrap_middleware)
        .get("/count", state_handler);

    let addr = start_server(app).await;

    let (status, headers, body) = http_get(addr, "/count").await;
    assert_eq!(status, 200);
    assert_eq!(body, "hits: 1");
    assert_eq!(header_val(&headers, "x-wrap"), Some("true"));
}
