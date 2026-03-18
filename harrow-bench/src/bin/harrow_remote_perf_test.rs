//! Remote performance test orchestrator.
//!
//! Runs from the LAPTOP, orchestrating everything via SSH:
//!   - SSHs to the server to start/stop Docker containers
//!   - SSHs to the client to run spinr (load tester) via Docker
//!   - Collects results via SSH stdout
//!   - Generates a markdown summary locally
//!
//! No inter-node SSH needed. The laptop drives both nodes.
//!
//! Three-phase bench run:
//!   Phase A — Serialization comparison (harrow bare vs axum bare)
//!   Phase B — Per-feature middleware overhead (harrow only)
//!   Phase C — O11y overhead (harrow only, with Vector)
//!
//! Usage:
//!   harrow-remote-perf-test --server-ssh IP --client-ssh IP --server-private IP --client-private IP
//!   harrow-remote-perf-test --server-ssh 52.1.2.3 --client-ssh 54.1.2.3 \
//!               --server-private 172.31.1.1 --client-private 172.31.1.2

use std::collections::BTreeMap;
use std::fs;
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_PORT: u16 = 3090;
const SLEEP_BETWEEN: Duration = Duration::from_secs(2);
const SSH_USER: &str = "alpine";

/// Phase A: serialization comparison endpoints (bare prefix, both frameworks).
const PHASE_A_ENDPOINTS: &[(&str, &str)] = &[
    ("bare/text", "text"),
    ("bare/json/1kb", "json_1kb"),
    ("bare/msgpack/1kb", "msgpack_1kb"),
];

const PHASE_A_CONCURRENCIES: &[u32] = &[1, 8, 32, 128];

/// Phase B: per-feature middleware overhead (harrow only).
const PHASE_B_PREFIXES: &[&str] = &[
    "bare",
    "timeout",
    "request-id",
    "cors",
    "compression",
    "full",
];

const PHASE_B_PAYLOADS: &[(&str, &str)] = &[
    ("text", "text"),
    ("json/1kb", "json_1kb"),
    ("msgpack/1kb", "msgpack_1kb"),
];

const PHASE_B_CONCURRENCIES: &[u32] = &[1, 32, 128];

/// Phase C: o11y overhead endpoints (same payloads as B, harrow only).
const PHASE_C_ENDPOINTS: &[(&str, &str)] = &[
    ("text", "text"),
    ("json/1kb", "json_1kb"),
    ("msgpack/1kb", "msgpack_1kb"),
];

const PHASE_C_CONCURRENCIES: &[u32] = &[1, 32, 128];

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

struct Args {
    /// Server public IP (for SSH from laptop)
    server_ssh: String,
    /// Client public IP (for SSH from laptop)
    client_ssh: String,
    /// Server private IP (for URLs from client over VPC)
    server_private: String,
    /// Client private IP (for OTLP endpoint from server in Phase C)
    client_private: String,
    /// SSH user for both nodes
    ssh_user: String,
    /// EC2 instance type (used in results path)
    instance_type: String,
    port: u16,
    duration: u32,
    warmup: u32,
    results_dir: std::path::PathBuf,
}

fn usage() -> ! {
    eprintln!(
        "Usage: harrow-remote-perf-test --server-ssh IP --client-ssh IP --server-private IP --client-private IP [OPTIONS]\n\
         \n\
         Three-phase benchmark suite (runs from laptop, drives both nodes via SSH):\n\
         \x20 Phase A — Serialization comparison (harrow bare vs axum bare)\n\
         \x20 Phase B — Per-feature middleware overhead (harrow only)\n\
         \x20 Phase C — O11y overhead (harrow only, with Vector)\n\
         \n\
         Required:\n\
         \x20 --server-ssh IP        Server public IP (for SSH)\n\
         \x20 --client-ssh IP        Client public IP (for SSH)\n\
         \x20 --server-private IP    Server private IP (for bench URLs over VPC)\n\
         \x20 --client-private IP    Client private IP (for OTLP endpoint in Phase C)\n\
         \n\
         Required:\n\
         \x20 --instance-type TYPE   EC2 instance type (e.g. c8g.16xlarge)\n\
         \n\
         Options:\n\
         \x20 --ssh-user USER        SSH user for both nodes (default: alpine)\n\
         \x20 --port PORT            Server port (default: 3090)\n\
         \x20 --duration SECS        Test duration per run (default: 60)\n\
         \x20 --warmup SECS          Warmup duration per run (default: 5)\n\
         \x20 --results-dir DIR      Override output directory (default: docs/perf/<instance-type>/<timestamp>)"
    );
    std::process::exit(1);
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut server_ssh: Option<String> = None;
    let mut client_ssh: Option<String> = None;
    let mut server_private: Option<String> = None;
    let mut client_private: Option<String> = None;
    let mut instance_type: Option<String> = None;
    let mut ssh_user = SSH_USER.to_string();
    let mut port: u16 = DEFAULT_PORT;
    let mut duration: u32 = 60;
    let mut warmup: u32 = 5;
    let mut results_dir_override: Option<std::path::PathBuf> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--server-ssh" => {
                server_ssh = Some(args[i + 1].clone());
                i += 2;
            }
            "--client-ssh" => {
                client_ssh = Some(args[i + 1].clone());
                i += 2;
            }
            "--server-private" => {
                server_private = Some(args[i + 1].clone());
                i += 2;
            }
            "--client-private" => {
                client_private = Some(args[i + 1].clone());
                i += 2;
            }
            "--instance-type" => {
                instance_type = Some(args[i + 1].clone());
                i += 2;
            }
            "--ssh-user" => {
                ssh_user = args[i + 1].clone();
                i += 2;
            }
            "--port" => {
                port = args[i + 1].parse().expect("invalid --port");
                i += 2;
            }
            "--duration" => {
                duration = args[i + 1].parse().expect("invalid --duration");
                i += 2;
            }
            "--warmup" => {
                warmup = args[i + 1].parse().expect("invalid --warmup");
                i += 2;
            }
            "--results-dir" => {
                results_dir_override = Some(std::path::PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown option: {other}");
                usage();
            }
        }
    }

    let require = |opt: Option<String>, name: &str| -> String {
        opt.unwrap_or_else(|| {
            eprintln!("error: {name} is required");
            usage();
        })
    };

    let server_ssh = require(server_ssh, "--server-ssh");
    let client_ssh = require(client_ssh, "--client-ssh");
    let server_private = require(server_private, "--server-private");
    let client_private = require(client_private, "--client-private");
    let instance_type = require(instance_type, "--instance-type");

    // Default results dir: docs/perf/<instance-type>/<YYYY-MM-DDTHH-MM-SSZ>/
    let results_dir = results_dir_override.unwrap_or_else(|| {
        let ts = Command::new("date")
            .args(["-u", "+%Y-%m-%dT%H-%M-%SZ"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".into());
        std::path::PathBuf::from(format!("docs/perf/{instance_type}/{ts}"))
    });

    Args {
        server_ssh,
        client_ssh,
        server_private,
        client_private,
        ssh_user,
        instance_type,
        port,
        duration,
        warmup,
        results_dir,
    }
}

// ---------------------------------------------------------------------------
// SSH helpers
// ---------------------------------------------------------------------------

fn ssh_run(user: &str, host: &str, remote_cmd: &str) -> std::io::Result<std::process::Output> {
    Command::new("ssh")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg("-o")
        .arg("ConnectTimeout=10")
        .arg(format!("{user}@{host}"))
        .arg(remote_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

fn ssh_server(args: &Args, remote_cmd: &str) -> std::io::Result<std::process::Output> {
    ssh_run(&args.ssh_user, &args.server_ssh, remote_cmd)
}

fn ssh_client(args: &Args, remote_cmd: &str) -> std::io::Result<std::process::Output> {
    ssh_run(&args.ssh_user, &args.client_ssh, remote_cmd)
}

// ---------------------------------------------------------------------------
// Server container management (via SSH to server)
// ---------------------------------------------------------------------------

fn start_server_container(args: &Args, name: &str, image: &str, docker_opts: &str, cmd_override: &str) {
    println!(">>> Starting container on server: {name}");
    let _ = ssh_server(args, &format!("docker rm -f {name} 2>/dev/null || true"));
    let docker_cmd = format!(
        "docker run -d --name {name} --network host --ulimit nofile=65535:65535 {docker_opts} {image} {cmd_override}"
    ).trim().to_string();
    let out = ssh_server(args, &docker_cmd);
    match out {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            eprintln!("  warning: docker run {name} stderr: {}", stderr.trim());
        }
        Err(e) => eprintln!("  failed to start container {name}: {e}"),
    }
    thread::sleep(Duration::from_secs(2));
}

fn stop_server_container(args: &Args, name: &str) {
    println!(">>> Stopping container on server: {name}");
    let _ = ssh_server(args, &format!("docker rm -f {name} 2>/dev/null || true"));
}

// ---------------------------------------------------------------------------
// Client container management (Vector, via SSH to client)
// ---------------------------------------------------------------------------

fn start_vector(args: &Args) {
    println!("--- Starting Vector on client (blackhole sink) ---");
    let _ = ssh_client(args, "docker rm -f vector 2>/dev/null || true");
    let cmd = "docker run -d --name vector --network host --ulimit nofile=65535:65535 \
               -v ~/vector.toml:/etc/vector/vector.toml:ro \
               timberio/vector:latest-alpine --config /etc/vector/vector.toml";
    let out = ssh_client(args, cmd);
    match out {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            eprintln!("  warning: vector start stderr: {}", stderr.trim());
        }
        Err(e) => eprintln!("  failed to start vector: {e}"),
    }
}

fn stop_vector(args: &Args) {
    println!("--- Stopping Vector on client ---");
    let _ = ssh_client(args, "docker rm -f vector 2>/dev/null || true");
}

// ---------------------------------------------------------------------------
// Health check (TCP connect from laptop)
// ---------------------------------------------------------------------------

fn wait_for_port(host: &str, port: u16, timeout: Duration) -> Result<(), String> {
    println!("    Waiting for {host}:{port}...");
    let deadline = Instant::now() + timeout;
    let addr = format!("{host}:{port}");
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(500)).is_ok() {
            println!("    Health check passed");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(format!("server on {addr} did not start within {timeout:?}"))
}

/// Health check via SSH for ports not reachable from the laptop (e.g. Vector 4318).
fn wait_for_port_via_ssh(args: &Args, host: &str, port: u16, timeout_secs: u32) -> Result<(), String> {
    println!("    Waiting for localhost:{port} on {host} (via SSH)...");
    let check_cmd = format!(
        "for i in $(seq 1 {timeout_secs}); do \
           nc -z -w1 127.0.0.1 {port} 2>/dev/null && echo ok && exit 0; \
           sleep 1; \
         done; exit 1"
    );
    let out = ssh_run(&args.ssh_user, host, &check_cmd);
    match out {
        Ok(o) if o.status.success() => {
            println!("    Health check passed");
            Ok(())
        }
        _ => Err(format!("localhost:{port} on {host} did not start within {timeout_secs}s")),
    }
}

// ---------------------------------------------------------------------------
// Bench runner (via SSH to client, runs spinr in Docker)
// ---------------------------------------------------------------------------

fn run_bench(
    args: &Args,
    url: &str,
    concurrency: u32,
    duration: u32,
    warmup: u32,
    outfile: &std::path::Path,
) -> Option<Value> {
    let remote_cmd = format!(
        "docker run --rm --network host --ulimit nofile=65535:65535 spinr load-test \
         --max-throughput -c {concurrency} -d {duration} -w {warmup} -j {url}"
    );
    let output = ssh_client(args, &remote_cmd);

    match output {
        Ok(o) if o.status.success() => {
            let _ = fs::write(outfile, &o.stdout);
            let val: Option<Value> = serde_json::from_slice(&o.stdout).ok();
            if let Some(ref v) = val {
                let rps = val_str(v, "rps");
                let p99 = val_str(v, "latency_p99_ms");
                println!("    → rps={rps} p99={p99}ms");
            }
            val
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            eprintln!("    bench failed (exit {}): {}", o.status, stderr.trim());
            None
        }
        Err(e) => {
            eprintln!("    failed to run bench: {e}");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Stats / logs collection (via SSH to server)
// ---------------------------------------------------------------------------

fn collect_docker_stats(args: &Args, label: &str) {
    let remote_cmd =
        "docker stats --no-stream --format '{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}'";
    if let Ok(out) = ssh_server(args, remote_cmd) {
        let path = args.results_dir.join(format!("stats_{label}.txt"));
        let _ = fs::write(path, &out.stdout);
    }
}

fn collect_docker_logs(args: &Args, container: &str, label: &str) {
    let remote_cmd = format!("docker logs {container} 2>&1");
    if let Ok(out) = ssh_server(args, &remote_cmd) {
        let path = args.results_dir.join(format!("logs_{label}.txt"));
        let _ = fs::write(path, &out.stdout);
    }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

fn generate_report(results: &BTreeMap<String, Value>, args: &Args) {
    let now = chrono_lite_utc();
    let mut report = format!(
        "# Performance Test Results\n\
         \n\
         Instance: {}\n\
         Server: {} (private: {}:{})\n\
         Client: {} (private: {})\n\
         Duration: {}s | Warmup: {}s\n\
         Date: {now}\n",
        args.instance_type,
        args.server_ssh, args.server_private, args.port,
        args.client_ssh, args.client_private,
        args.duration, args.warmup,
    );

    // Phase A
    report.push_str("\n## Phase A: Serialization Comparison (harrow bare vs axum bare)\n\n");
    report.push_str(
        "| Framework | Endpoint | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |\n",
    );
    report.push_str(
        "|-----------|----------|-------------|-----|----------|----------|----------|\n",
    );

    for fw in ["harrow", "axum"] {
        for &(path, label) in PHASE_A_ENDPOINTS {
            for &c in PHASE_A_CONCURRENCIES {
                let key = format!("a_{fw}_{label}_c{c}");
                let (rps, p50, p99, p999) = extract_latencies(results.get(&key));
                report.push_str(&format!(
                    "| {fw} | /{path} | {c} | {rps} | {p50} | {p99} | {p999} |\n"
                ));
            }
        }
    }

    // Phase B
    report.push_str("\n## Phase B: Per-Feature Middleware Overhead (harrow only)\n\n");
    report.push_str(
        "| Feature | Payload | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |\n",
    );
    report
        .push_str("|---------|---------|-------------|-----|----------|----------|----------|\n");

    for &prefix in PHASE_B_PREFIXES {
        for &(payload, label) in PHASE_B_PAYLOADS {
            for &c in PHASE_B_CONCURRENCIES {
                let key = format!("b_{prefix}_{label}_c{c}");
                let (rps, p50, p99, p999) = extract_latencies(results.get(&key));
                report.push_str(&format!(
                    "| {prefix} | /{payload} | {c} | {rps} | {p50} | {p99} | {p999} |\n"
                ));
            }
        }
    }

    // Phase C
    report.push_str("\n## Phase C: O11y Overhead (harrow only, with Vector)\n\n");
    report
        .push_str("| Endpoint | Concurrency | RPS | p50 (ms) | p99 (ms) | p999 (ms) |\n");
    report.push_str("|----------|-------------|-----|----------|----------|----------|\n");

    for &(path, label) in PHASE_C_ENDPOINTS {
        for &c in PHASE_C_CONCURRENCIES {
            let key = format!("c_o11y_{label}_c{c}");
            let (rps, p50, p99, p999) = extract_latencies(results.get(&key));
            report.push_str(&format!(
                "| /{path} | {c} | {rps} | {p50} | {p99} | {p999} |\n"
            ));
        }
    }

    let report_path = args.results_dir.join("summary.md");
    fs::write(&report_path, &report).unwrap();
    println!("Summary written to {}", report_path.display());
}

fn extract_latencies(v: Option<&Value>) -> (String, String, String, String) {
    match v {
        Some(v) => (
            val_str(v, "rps"),
            val_str(v, "latency_p50_ms"),
            val_str(v, "latency_p99_ms"),
            val_str(v, "latency_p999_ms"),
        ),
        None => ("-".into(), "-".into(), "-".into(), "-".into()),
    }
}

fn val_str(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::Number(n)) => {
            if let Some(f) = n.as_f64() {
                if f == f.floor() && f.abs() < 1e15 {
                    format!("{}", f as i64)
                } else {
                    format!("{f:.3}")
                }
            } else {
                n.to_string()
            }
        }
        Some(v) => v.to_string(),
        None => "-".into(),
    }
}

/// Minimal UTC timestamp without pulling in chrono.
fn chrono_lite_utc() -> String {
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%d %H:%M:%S UTC"])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "unknown".into(),
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Preflight checks
// ---------------------------------------------------------------------------

fn preflight_checks(args: &Args) {
    println!("--- Preflight checks ---");

    // Verify SSH connectivity
    for (label, host) in [("server", &args.server_ssh), ("client", &args.client_ssh)] {
        let out = ssh_run(&args.ssh_user, host, "echo ok");
        match out {
            Ok(o) if o.status.success() => println!("  SSH to {label} ({host}): ok"),
            _ => {
                eprintln!("  SSH to {label} ({host}): FAILED");
                std::process::exit(1);
            }
        }
    }

    // Verify Docker is running on both nodes
    for (label, host) in [("server", &args.server_ssh), ("client", &args.client_ssh)] {
        let out = ssh_run(&args.ssh_user, host, "docker info >/dev/null 2>&1 && echo ok");
        match out {
            Ok(o) if o.status.success() => println!("  Docker on {label}: ok"),
            _ => {
                eprintln!("  Docker on {label} ({host}): FAILED — is Docker running?");
                std::process::exit(1);
            }
        }
    }

    // Verify ulimits inside Docker containers
    for (label, host) in [("server", &args.server_ssh), ("client", &args.client_ssh)] {
        let out = ssh_run(
            &args.ssh_user, host,
            "docker run --rm --ulimit nofile=65535:65535 alpine sh -c 'ulimit -n'"
        );
        match out {
            Ok(o) if o.status.success() => {
                let val = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if val == "65535" {
                    println!("  Container ulimit on {label}: {val} (ok)");
                } else {
                    eprintln!("  WARNING: Container ulimit on {label}: {val} (expected 65535)");
                }
            }
            _ => eprintln!("  WARNING: Could not verify container ulimit on {label}"),
        }
    }

    // Verify required images on server
    for image in ["harrow-perf-server", "axum-perf-server"] {
        let out = ssh_run(&args.ssh_user, &args.server_ssh, &format!("docker image inspect {image} >/dev/null 2>&1 && echo ok"));
        match out {
            Ok(o) if o.status.success() => println!("  Image {image} on server: ok"),
            _ => {
                eprintln!("  Image {image} on server: MISSING");
                std::process::exit(1);
            }
        }
    }

    // Verify required images on client
    for image in ["spinr", "timberio/vector:latest-alpine"] {
        let out = ssh_run(&args.ssh_user, &args.client_ssh, &format!("docker image inspect {image} >/dev/null 2>&1 && echo ok"));
        match out {
            Ok(o) if o.status.success() => println!("  Image {image} on client: ok"),
            _ => {
                eprintln!("  Image {image} on client: MISSING");
                std::process::exit(1);
            }
        }
    }

    // Verify server port is reachable from laptop
    println!("  Checking port {} reachable on server public IP...", args.port);

    println!("--- Preflight checks passed ---");
    println!();
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args = parse_args();
    fs::create_dir_all(&args.results_dir).unwrap();

    preflight_checks(&args);

    println!("============================================");
    println!(" Harrow Performance Test Suite (3-phase)");
    println!(" Instance: {}", args.instance_type);
    println!(" Server: {} (private: {}:{})", args.server_ssh, args.server_private, args.port);
    println!(" Client: {} (private: {})", args.client_ssh, args.client_private);
    println!(" Duration: {}s  Warmup: {}s", args.duration, args.warmup);
    println!(" Results: {}/", args.results_dir.display());
    println!("============================================");
    println!();

    let mut results: BTreeMap<String, Value> = BTreeMap::new();

    // -----------------------------------------------------------------------
    // Phase A: Serialization comparison (harrow bare vs axum bare)
    // -----------------------------------------------------------------------
    println!("========== PHASE A: Serialization comparison ==========");

    // --- Harrow (bare group, no middleware) ---
    println!();
    println!("--- Harrow (bare) ---");
    start_server_container(&args, "harrow-perf-server", "harrow-perf-server", "", "");
    if let Err(e) = wait_for_port(&args.server_ssh, args.port, Duration::from_secs(30)) {
        eprintln!("  {e}");
        stop_server_container(&args, "harrow-perf-server");
        std::process::exit(1);
    }

    for &(path, label) in PHASE_A_ENDPOINTS {
        for &c in PHASE_A_CONCURRENCIES {
            let url = format!("http://{}:{}/{path}", args.server_private, args.port);
            let key = format!("a_harrow_{label}_c{c}");
            let outfile = args.results_dir.join(format!("{key}.json"));
            println!("  [{key}] c={c} → {url}");
            if let Some(v) = run_bench(&args, &url, c, args.duration, args.warmup, &outfile) {
                results.insert(key, v);
            }
            thread::sleep(SLEEP_BETWEEN);
        }
    }

    collect_docker_stats(&args, "harrow_bare");
    collect_docker_logs(&args, "harrow-perf-server", "harrow_bare");
    stop_server_container(&args, "harrow-perf-server");

    // --- Axum (bare group, no middleware) ---
    println!();
    println!("--- Axum (bare) ---");
    start_server_container(&args, "axum-perf-server", "axum-perf-server", "", "");
    if let Err(e) = wait_for_port(&args.server_ssh, args.port, Duration::from_secs(30)) {
        eprintln!("  {e}");
        stop_server_container(&args, "axum-perf-server");
        std::process::exit(1);
    }

    for &(path, label) in PHASE_A_ENDPOINTS {
        for &c in PHASE_A_CONCURRENCIES {
            let url = format!("http://{}:{}/{path}", args.server_private, args.port);
            let key = format!("a_axum_{label}_c{c}");
            let outfile = args.results_dir.join(format!("{key}.json"));
            println!("  [{key}] c={c} → {url}");
            if let Some(v) = run_bench(&args, &url, c, args.duration, args.warmup, &outfile) {
                results.insert(key, v);
            }
            thread::sleep(SLEEP_BETWEEN);
        }
    }

    collect_docker_stats(&args, "axum_bare");
    collect_docker_logs(&args, "axum-perf-server", "axum_bare");
    stop_server_container(&args, "axum-perf-server");

    // -----------------------------------------------------------------------
    // Phase B: Per-feature middleware overhead (harrow only)
    // -----------------------------------------------------------------------
    println!();
    println!("========== PHASE B: Per-feature middleware overhead ==========");

    start_server_container(&args, "harrow-perf-server", "harrow-perf-server", "", "");
    if let Err(e) = wait_for_port(&args.server_ssh, args.port, Duration::from_secs(30)) {
        eprintln!("  {e}");
        stop_server_container(&args, "harrow-perf-server");
        std::process::exit(1);
    }

    for &prefix in PHASE_B_PREFIXES {
        println!();
        println!("--- {prefix} ---");
        for &(payload, label) in PHASE_B_PAYLOADS {
            for &c in PHASE_B_CONCURRENCIES {
                let url = format!(
                    "http://{}:{}/{prefix}/{payload}",
                    args.server_private, args.port
                );
                let key = format!("b_{prefix}_{label}_c{c}");
                let outfile = args.results_dir.join(format!("{key}.json"));
                println!("  [{key}] c={c} → {url}");
                if let Some(v) = run_bench(&args, &url, c, args.duration, args.warmup, &outfile) {
                    results.insert(key, v);
                }
                thread::sleep(SLEEP_BETWEEN);
            }
        }
    }

    collect_docker_stats(&args, "harrow_middleware");
    collect_docker_logs(&args, "harrow-perf-server", "harrow_middleware");
    stop_server_container(&args, "harrow-perf-server");

    // -----------------------------------------------------------------------
    // Phase C: O11y overhead (harrow only, with Vector)
    // -----------------------------------------------------------------------
    println!();
    println!("========== PHASE C: O11y overhead (Harrow) ==========");

    start_vector(&args);

    println!("  Waiting for Vector to be ready...");
    // Vector port 4318 is only reachable within VPC, check via SSH
    if let Err(e) = wait_for_port_via_ssh(&args, &args.client_ssh, 4318, 30) {
        eprintln!("  {e}");
        stop_vector(&args);
        std::process::exit(1);
    }

    let o11y_env = format!("-e OTLP_ENDPOINT=http://{}:4318", args.client_private);
    start_server_container(&args, "harrow-perf-o11y", "harrow-perf-server", &o11y_env, "/harrow-perf-server --bind 0.0.0.0 --o11y");
    if let Err(e) = wait_for_port(&args.server_ssh, args.port, Duration::from_secs(30)) {
        eprintln!("  {e}");
        stop_server_container(&args, "harrow-perf-o11y");
        stop_vector(&args);
        std::process::exit(1);
    }

    for &(path, label) in PHASE_C_ENDPOINTS {
        for &c in PHASE_C_CONCURRENCIES {
            let url = format!("http://{}:{}/{path}", args.server_private, args.port);
            let key = format!("c_o11y_{label}_c{c}");
            let outfile = args.results_dir.join(format!("{key}.json"));
            println!("  [{key}] c={c} → {url}");
            if let Some(v) = run_bench(&args, &url, c, args.duration, args.warmup, &outfile) {
                results.insert(key, v);
            }
            thread::sleep(SLEEP_BETWEEN);
        }
    }

    collect_docker_stats(&args, "harrow_o11y");
    collect_docker_logs(&args, "harrow-perf-o11y", "harrow_o11y");
    stop_server_container(&args, "harrow-perf-o11y");
    stop_vector(&args);

    // -----------------------------------------------------------------------
    // Summary
    // -----------------------------------------------------------------------
    println!();
    println!("========== GENERATING SUMMARY ==========");
    generate_report(&results, &args);
    println!();
    println!("Done! Results in {}/", args.results_dir.display());
}
