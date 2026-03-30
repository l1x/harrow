use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use crate::perf_summary;

use super::schema::{
    BenchmarkMetrics, CaseReport, CompareSide, GitDescriptor, ImageDescriptor,
    ImplementationLabels, ImplementationResult, LatencyMetrics, LoadProfile, RUN_SCHEMA_VERSION,
    ResultArtifacts, RunDefaults, RunReport, SuiteDescriptor, TargetDescriptor, TemplateFiles,
};
use super::spec::{
    CaseSpec, DeploymentMode, ImplementationRegistry, ImplementationSpec, LoadGeneratorKind,
    RunMode, SuiteSpec,
};
use super::template::{render_template_file, render_template_str};

const DEFAULT_PORT: u16 = 3090;
const DEFAULT_SSH_USER: &str = "alpine";
const DEFAULT_SPINR_IMAGE: &str = "spinr:arm64";
const DEFAULT_SPINR_BUILD_TASK: &str = "docker:loadgen:spinr";
const DEFAULT_VEGETA_IMAGE: &str = "vegeta:arm64";
const DEFAULT_VEGETA_BUILD_TASK: &str = "docker:loadgen:vegeta";
const SLEEP_BETWEEN_RUNS: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct CommonRunConfig {
    pub deployment_mode: DeploymentMode,
    pub suite_path: PathBuf,
    pub registry_path: PathBuf,
    pub case_filters: Vec<String>,
    pub results_dir: Option<PathBuf>,
    pub server_ssh: Option<String>,
    pub client_ssh: Option<String>,
    pub server_private_ip: Option<String>,
    pub ssh_user: String,
    pub port: u16,
    pub duration_secs: u32,
    pub warmup_secs: u32,
    pub build_missing: bool,
}

impl Default for CommonRunConfig {
    fn default() -> Self {
        Self {
            deployment_mode: DeploymentMode::Local,
            suite_path: PathBuf::from("harrow-bench/suites/http-basic.toml"),
            registry_path: PathBuf::from("harrow-bench/implementations.toml"),
            case_filters: Vec::new(),
            results_dir: None,
            server_ssh: None,
            client_ssh: None,
            server_private_ip: None,
            ssh_user: DEFAULT_SSH_USER.to_string(),
            port: DEFAULT_PORT,
            duration_secs: 30,
            warmup_secs: 5,
            build_missing: true,
        }
    }
}

#[derive(Clone)]
pub struct SingleRunConfig {
    pub common: CommonRunConfig,
    pub implementation_id: String,
}

#[derive(Clone)]
pub struct CompareRunConfig {
    pub common: CommonRunConfig,
    pub left_id: String,
    pub right_id: String,
}

#[derive(Clone)]
struct RunVariant {
    implementation: ImplementationSpec,
    variant_label: String,
    compare_side: Option<CompareSide>,
}

#[derive(Clone, Debug, Deserialize)]
struct VegetaMetrics {
    #[serde(rename = "latencies")]
    latencies: VegetaLatencies,
    #[serde(rename = "throughput")]
    throughput: f64,
    #[serde(rename = "success")]
    success: f64,
    #[serde(rename = "status_codes")]
    status_codes: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Deserialize)]
struct VegetaLatencies {
    #[serde(rename = "mean")]
    mean: f64,
    #[serde(rename = "50th")]
    p50: f64,
    #[serde(rename = "95th")]
    p95: f64,
    #[serde(rename = "99th")]
    p99: f64,
    #[serde(rename = "max")]
    max: f64,
}

struct BenchRunResult {
    value: Option<Value>,
    error: Option<String>,
    raw_output_path: PathBuf,
}

#[derive(Clone)]
struct CaseExecutionRecord {
    case: CaseSpec,
    rendered_template: PathBuf,
    results: Vec<ImplementationExecutionRecord>,
}

#[derive(Clone)]
struct ImplementationExecutionRecord {
    variant: RunVariant,
    started_at_utc: String,
    completed_at_utc: String,
    image_id: Option<String>,
    raw_metrics_path: PathBuf,
    metrics: Value,
}

pub fn run_single(config: SingleRunConfig) -> Result<PathBuf, String> {
    let common = config.common.clone();
    let registry = ImplementationRegistry::load(&common.registry_path)?;
    let implementation = registry
        .get(&config.implementation_id)
        .cloned()
        .ok_or_else(|| format!("unknown implementation '{}'", config.implementation_id))?;

    run_plan(
        RunMode::Single,
        common,
        vec![RunVariant {
            variant_label: implementation.id.clone(),
            implementation,
            compare_side: None,
        }],
    )
}

pub fn run_compare(config: CompareRunConfig) -> Result<PathBuf, String> {
    let common = config.common.clone();
    let registry = ImplementationRegistry::load(&common.registry_path)?;

    let left = registry
        .get(&config.left_id)
        .cloned()
        .ok_or_else(|| format!("unknown implementation '{}'", config.left_id))?;
    let right = registry
        .get(&config.right_id)
        .cloned()
        .ok_or_else(|| format!("unknown implementation '{}'", config.right_id))?;

    run_plan(
        RunMode::Compare,
        common,
        vec![
            RunVariant {
                variant_label: left.id.clone(),
                implementation: left,
                compare_side: Some(CompareSide::Left),
            },
            RunVariant {
                variant_label: right.id.clone(),
                implementation: right,
                compare_side: Some(CompareSide::Right),
            },
        ],
    )
}

fn run_plan(
    mode: RunMode,
    config: CommonRunConfig,
    variants: Vec<RunVariant>,
) -> Result<PathBuf, String> {
    validate_common_config(&config)?;

    let suite = SuiteSpec::load(&config.suite_path)?;
    let selected_cases = suite.selected_cases(&config.case_filters)?;
    if selected_cases.is_empty() {
        return Err(format!("suite '{}' contains no runnable cases", suite.name));
    }

    let results_dir = config
        .results_dir
        .clone()
        .unwrap_or_else(|| default_results_dir(mode, &suite, &variants));
    fs::create_dir_all(results_dir.join("rendered")).map_err(|e| {
        format!(
            "failed to create results dir {}: {e}",
            results_dir.display()
        )
    })?;

    preflight_checks(&config, &variants, selected_cases.as_slice())?;

    let started_at_utc = chrono_lite_utc();
    let mut case_records = Vec::with_capacity(selected_cases.len());

    print_run_header(
        mode,
        &config,
        &suite,
        &variants,
        selected_cases.as_slice(),
        &results_dir,
    );

    for case in selected_cases {
        println!("========== TARGET: {} ==========", case.id);
        let rendered_template = render_case_template(&config, &suite, case, &results_dir)?;

        let mut implementation_records = Vec::with_capacity(variants.len());
        for variant in &variants {
            println!();
            println!("--- {} / {} ---", variant.variant_label, case.id);
            let record =
                run_case_for_variant(&config, case, &rendered_template, variant, &results_dir)?;
            implementation_records.push(record);
            thread::sleep(SLEEP_BETWEEN_RUNS);
        }

        case_records.push(CaseExecutionRecord {
            case: case.clone(),
            rendered_template,
            results: implementation_records,
        });
    }

    let completed_at_utc = chrono_lite_utc();
    write_canonical_report(
        mode,
        &config,
        &suite,
        &results_dir,
        &started_at_utc,
        &completed_at_utc,
        &case_records,
    )?;

    println!();
    println!("========== GENERATING SUMMARY ==========");
    perf_summary::render_results_dir(&results_dir)
        .map_err(|e| format!("failed to render summary in {}: {e}", results_dir.display()))?;
    println!(
        "Summary written to {}",
        results_dir.join("summary.md").display()
    );
    println!();
    println!("Done! Results in {}/", results_dir.display());

    Ok(results_dir)
}

fn validate_common_config(config: &CommonRunConfig) -> Result<(), String> {
    match config.deployment_mode {
        DeploymentMode::Local => Ok(()),
        DeploymentMode::Remote => {
            if config.server_ssh.is_none()
                || config.client_ssh.is_none()
                || config.server_private_ip.is_none()
            {
                return Err(
                    "--server-ssh, --client-ssh, and --server-private-ip are required in remote mode".into(),
                );
            }
            Ok(())
        }
    }
}

fn default_results_dir(mode: RunMode, suite: &SuiteSpec, variants: &[RunVariant]) -> PathBuf {
    let ts = timestamp_slug();
    let label = match mode {
        RunMode::Single => sanitize_label(&variants[0].variant_label),
        RunMode::Compare => format!(
            "{}-vs-{}",
            sanitize_label(&variants[0].variant_label),
            sanitize_label(&variants[1].variant_label)
        ),
    };
    PathBuf::from(format!(
        "artifacts/bench/{ts}-{}-{}",
        sanitize_label(&suite.name),
        label
    ))
}

fn print_run_header(
    mode: RunMode,
    config: &CommonRunConfig,
    suite: &SuiteSpec,
    variants: &[RunVariant],
    cases: &[&CaseSpec],
    results_dir: &Path,
) {
    println!("============================================");
    println!(" Benchmark Harness :: {}", mode.as_str());
    println!(" Suite: {}", suite.name);
    if variants.len() == 1 {
        println!(" Implementation: {}", variants[0].variant_label);
    } else {
        println!(
            " Comparison: {} vs {}",
            variants[0].variant_label, variants[1].variant_label
        );
    }
    println!(" Mode: {}", config.deployment_mode.as_str());
    match config.deployment_mode {
        DeploymentMode::Local => {
            println!(
                " Server target: {}:{}",
                config.server_private_ip.as_deref().unwrap_or("127.0.0.1"),
                config.port
            );
        }
        DeploymentMode::Remote => {
            println!(
                " Server: {} (private: {}:{})",
                config.server_ssh.as_deref().unwrap_or("unknown"),
                config.server_private_ip.as_deref().unwrap_or("unknown"),
                config.port
            );
            println!(
                " Client: {}",
                config.client_ssh.as_deref().unwrap_or("unknown")
            );
        }
    }
    println!(
        " Duration: {}s  Warmup: {}s",
        config.duration_secs, config.warmup_secs
    );
    println!(
        " Cases: {}",
        cases
            .iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(" Results: {}/", results_dir.display());
    println!("============================================");
    println!();
}

fn render_case_template(
    config: &CommonRunConfig,
    suite: &SuiteSpec,
    case: &CaseSpec,
    results_dir: &Path,
) -> Result<PathBuf, String> {
    let template_path = case.resolved_template_path(&config.suite_path);
    let context = template_context(config, suite, case);
    let rendered = render_template_file(&template_path, &context)?;

    let suffix = match case.generator {
        LoadGeneratorKind::Spinr => "toml",
        LoadGeneratorKind::Vegeta => "targets.txt",
    };
    let rendered_path = results_dir
        .join("rendered")
        .join(format!("{}.{}", case.id, suffix));
    fs::write(&rendered_path, rendered).map_err(|e| {
        format!(
            "failed to write rendered template {}: {e}",
            rendered_path.display()
        )
    })?;
    Ok(rendered_path)
}

fn template_context(
    config: &CommonRunConfig,
    suite: &SuiteSpec,
    case: &CaseSpec,
) -> BTreeMap<String, Value> {
    let mut context = BTreeMap::new();
    let server_private_ip = config
        .server_private_ip
        .clone()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let duration_secs = case.duration_secs.unwrap_or(config.duration_secs);
    let warmup_secs = case.warmup_secs.unwrap_or(config.warmup_secs);

    context.insert("suite".into(), Value::String(suite.name.clone()));
    context.insert("case_id".into(), Value::String(case.id.clone()));
    context.insert(
        "server_private_ip".into(),
        Value::String(server_private_ip.clone()),
    );
    context.insert("port".into(), Value::Number(config.port.into()));
    context.insert(
        "base_url".into(),
        Value::String(format!("http://{server_private_ip}:{}", config.port)),
    );
    context.insert("duration_secs".into(), Value::Number(duration_secs.into()));
    context.insert("warmup_secs".into(), Value::Number(warmup_secs.into()));
    if let Some(rate) = case.rate {
        context.insert("rate".into(), Value::Number(rate.into()));
    }
    if let Some(concurrency) = resolved_concurrency(case) {
        context.insert("connections".into(), Value::Number(concurrency.into()));
        context.insert("concurrency".into(), Value::Number(concurrency.into()));
        context.insert("workers".into(), Value::Number(concurrency.into()));
    }

    for (key, value) in &case.context {
        if let Ok(json_value) = serde_json::to_value(value) {
            context.insert(key.clone(), json_value);
        }
    }

    context
}

fn run_case_for_variant(
    config: &CommonRunConfig,
    case: &CaseSpec,
    rendered_template: &Path,
    variant: &RunVariant,
    results_dir: &Path,
) -> Result<ImplementationExecutionRecord, String> {
    start_server_container(config, case, variant)?;
    if let Err(error) = wait_for_server(config, &variant.implementation, Duration::from_secs(30)) {
        stop_server_container(config, variant);
        return Err(error);
    }

    let artifacts_dir = results_dir
        .join("raw")
        .join(sanitize_label(&case.id))
        .join(sanitize_label(&variant.implementation.id));
    if let Err(error) = fs::create_dir_all(&artifacts_dir) {
        stop_server_container(config, variant);
        return Err(format!(
            "failed to create raw artifact dir {}: {error}",
            artifacts_dir.display()
        ));
    }

    let key = run_key(&variant.variant_label, &case.id);
    let raw_metrics_path = artifacts_dir.join("loadgen.json");
    let started_at_utc = chrono_lite_utc();

    let run_result = match case.generator {
        LoadGeneratorKind::Spinr => {
            run_spinr_bench(config, &key, rendered_template, &raw_metrics_path)
        }
        LoadGeneratorKind::Vegeta => {
            run_vegeta_bench(config, case, &key, rendered_template, &raw_metrics_path)
        }
    };

    let completed_at_utc = chrono_lite_utc();
    stop_server_container(config, variant);

    if let Some(error) = run_result.error {
        return Err(error);
    }

    let metrics = run_result
        .value
        .clone()
        .ok_or_else(|| format!("run '{}' completed without parsed metrics", key))?;
    let image_id = image_id(config, &variant.implementation.image);

    Ok(ImplementationExecutionRecord {
        variant: variant.clone(),
        started_at_utc,
        completed_at_utc,
        image_id,
        raw_metrics_path: run_result.raw_output_path,
        metrics,
    })
}

fn preflight_checks(
    config: &CommonRunConfig,
    variants: &[RunVariant],
    cases: &[&CaseSpec],
) -> Result<(), String> {
    println!("--- Preflight checks ---");

    match run_local("docker info >/dev/null 2>&1 && echo ok") {
        Ok(out) if out.status.success() => println!("  Docker: ok"),
        _ => return Err("Docker is not available locally".into()),
    }

    match config.deployment_mode {
        DeploymentMode::Local => {}
        DeploymentMode::Remote => {
            for (label, host) in [
                ("server", config.server_ssh.as_deref().unwrap_or("")),
                ("client", config.client_ssh.as_deref().unwrap_or("")),
            ] {
                let out = ssh_run(&config.ssh_user, host, "echo ok")
                    .map_err(|e| format!("failed to reach {label} host {host}: {e}"))?;
                if !out.status.success() {
                    return Err(format!("SSH to {label} host {host} failed"));
                }
                println!("  SSH to {label} ({host}): ok");
            }

            for (label, cmd) in [
                (
                    "server docker",
                    ssh_server(config, "docker info >/dev/null 2>&1 && echo ok"),
                ),
                (
                    "client docker",
                    ssh_client(config, "docker info >/dev/null 2>&1 && echo ok"),
                ),
            ] {
                let out = cmd.map_err(|e| format!("failed to check {label}: {e}"))?;
                if !out.status.success() {
                    return Err(format!("{label} check failed"));
                }
                println!("  {label}: ok");
            }
        }
    }

    for variant in variants {
        ensure_server_image(config, &variant.implementation)?;
        println!("  Image {}: ok", variant.implementation.image);
    }

    for case in cases {
        ensure_loadgen_image(config, case.generator)?;
        println!("  Load generator {}: ok", case.generator.as_str());
    }

    println!("--- Preflight checks passed ---");
    println!();
    Ok(())
}

fn ensure_server_image(
    config: &CommonRunConfig,
    implementation: &ImplementationSpec,
) -> Result<(), String> {
    let inspect_cmd = format!(
        "docker image inspect {} >/dev/null 2>&1 && echo ok",
        implementation.image
    );
    match config.deployment_mode {
        DeploymentMode::Local => {
            let out = run_local(&inspect_cmd).map_err(|e| {
                format!(
                    "failed to inspect local image {}: {e}",
                    implementation.image
                )
            })?;
            if out.status.success() {
                return Ok(());
            }

            if config.build_missing
                && let Some(task) = implementation.build_task.as_deref()
            {
                println!(
                    "  Building missing image {} via {}",
                    implementation.image, task
                );
                let build = run_local(&format!("mise run {task}"))
                    .map_err(|e| format!("failed to run local build task {task}: {e}"))?;
                if !build.status.success() {
                    let stderr = String::from_utf8_lossy(&build.stderr);
                    return Err(format!("local build task {task} failed: {}", stderr.trim()));
                }
                let recheck = run_local(&inspect_cmd).map_err(|e| {
                    format!("failed to re-check image {}: {e}", implementation.image)
                })?;
                if recheck.status.success() {
                    return Ok(());
                }
            }

            Err(format!(
                "missing local image '{}' and no successful build path was available",
                implementation.image
            ))
        }
        DeploymentMode::Remote => {
            let out = ssh_server(config, &inspect_cmd).map_err(|e| {
                format!(
                    "failed to inspect remote image {}: {e}",
                    implementation.image
                )
            })?;
            if out.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "missing remote image '{}' on server host",
                    implementation.image
                ))
            }
        }
    }
}

fn ensure_loadgen_image(
    config: &CommonRunConfig,
    generator: LoadGeneratorKind,
) -> Result<(), String> {
    let (image, build_task) = match generator {
        LoadGeneratorKind::Spinr => (DEFAULT_SPINR_IMAGE, Some(DEFAULT_SPINR_BUILD_TASK)),
        LoadGeneratorKind::Vegeta => (DEFAULT_VEGETA_IMAGE, Some(DEFAULT_VEGETA_BUILD_TASK)),
    };

    let inspect_cmd = format!("docker image inspect {image} >/dev/null 2>&1 && echo ok");
    match config.deployment_mode {
        DeploymentMode::Local => {
            let out = run_local(&inspect_cmd).map_err(|e| {
                format!("failed to inspect local load generator image {image}: {e}")
            })?;
            if out.status.success() {
                return Ok(());
            }

            if config.build_missing
                && let Some(task) = build_task
            {
                println!("  Building missing image {image} via {task}");
                let build = run_local(&format!("mise run {task}"))
                    .map_err(|e| format!("failed to run load generator build task {task}: {e}"))?;
                if !build.status.success() {
                    let stderr = String::from_utf8_lossy(&build.stderr);
                    return Err(format!(
                        "load generator build task {task} failed: {}",
                        stderr.trim()
                    ));
                }

                let recheck = run_local(&inspect_cmd)
                    .map_err(|e| format!("failed to re-check image {image}: {e}"))?;
                if recheck.status.success() {
                    return Ok(());
                }
            }

            Err(format!("missing local load generator image '{image}'"))
        }
        DeploymentMode::Remote => {
            let out = ssh_client(config, &inspect_cmd).map_err(|e| {
                format!("failed to inspect remote load generator image {image}: {e}")
            })?;
            if out.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "missing remote load generator image '{image}' on client host"
                ))
            }
        }
    }
}

fn start_server_container(
    config: &CommonRunConfig,
    case: &CaseSpec,
    variant: &RunVariant,
) -> Result<(), String> {
    let container_name = container_name(&variant.implementation.id);
    let mut context = BTreeMap::new();
    context.insert("port".into(), Value::Number(config.port.into()));
    let base_command = render_template_str(&variant.implementation.command, &context)?;
    let command = if case.server_flags.is_empty() {
        base_command
    } else {
        format!("{base_command} {}", case.server_flags.join(" "))
    };

    println!(
        ">>> Starting {} server on {}",
        variant.variant_label,
        config.deployment_mode.as_str()
    );

    match config.deployment_mode {
        DeploymentMode::Local => {
            let _ = run_local(&format!(
                "docker rm -f {container_name} >/dev/null 2>&1 || true"
            ));
            let docker_cmd = format!(
                "docker run -d --name {container_name} --network host --ulimit nofile=65535:65535 {} {}",
                variant.implementation.image, command
            );
            let out = run_local(&docker_cmd)
                .map_err(|e| format!("failed to start local container {container_name}: {e}"))?;
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(format!(
                    "failed to start local container {container_name}: {}",
                    stderr.trim()
                ));
            }
        }
        DeploymentMode::Remote => {
            let _ = ssh_server(
                config,
                &format!("docker rm -f {container_name} >/dev/null 2>&1 || true"),
            );
            let docker_cmd = format!(
                "docker run -d --name {container_name} --network host --ulimit nofile=65535:65535 {} {}",
                variant.implementation.image, command
            );
            let out = ssh_server(config, &docker_cmd)
                .map_err(|e| format!("failed to start remote container {container_name}: {e}"))?;
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(format!(
                    "failed to start remote container {container_name}: {}",
                    stderr.trim()
                ));
            }
        }
    }

    thread::sleep(Duration::from_secs(2));
    Ok(())
}

fn stop_server_container(config: &CommonRunConfig, variant: &RunVariant) {
    let container_name = container_name(&variant.implementation.id);
    println!(">>> Stopping {} server", variant.variant_label);
    match config.deployment_mode {
        DeploymentMode::Local => {
            let _ = run_local(&format!(
                "docker rm -f {container_name} >/dev/null 2>&1 || true"
            ));
        }
        DeploymentMode::Remote => {
            let _ = ssh_server(
                config,
                &format!("docker rm -f {container_name} >/dev/null 2>&1 || true"),
            );
        }
    }
}

fn wait_for_server(
    config: &CommonRunConfig,
    implementation: &ImplementationSpec,
    timeout: Duration,
) -> Result<(), String> {
    let host = config.server_private_ip.as_deref().unwrap_or("127.0.0.1");
    let addr = format!("{host}:{}", config.port);
    println!("    Waiting for {addr}...");

    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if http_health_check(host, config.port, implementation.health_path()) {
            println!("    Health endpoint passed");
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }

    Err(format!(
        "server on {addr} did not pass GET {} within {timeout:?}",
        implementation.health_path()
    ))
}

fn http_health_check(host: &str, port: u16, path: &str) -> bool {
    let addr = match format!("{host}:{port}").parse() {
        Ok(addr) => addr,
        Err(_) => return false,
    };

    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(500)) {
        Ok(stream) => stream,
        Err(_) => return false,
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut buf = [0u8; 256];
    let n = match stream.read(&mut buf) {
        Ok(n) if n > 0 => n,
        _ => return false,
    };
    let response = String::from_utf8_lossy(&buf[..n]);
    response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200")
}

fn run_spinr_bench(
    config: &CommonRunConfig,
    key: &str,
    rendered_template: &Path,
    output_path: &Path,
) -> BenchRunResult {
    let cmd = match config.deployment_mode {
        DeploymentMode::Local => format!(
            "docker run --rm --network host --ulimit nofile=65535:65535 -v {}:/bench.toml {DEFAULT_SPINR_IMAGE} bench /bench.toml -j",
            rendered_template.display()
        ),
        DeploymentMode::Remote => {
            let remote_config = format!("/tmp/{key}.toml");
            scp_to_client(config, rendered_template, &remote_config);
            format!(
                "docker run --rm --network host --ulimit nofile=65535:65535 -v {remote_config}:/bench.toml {DEFAULT_SPINR_IMAGE} bench /bench.toml -j"
            )
        }
    };

    let result = match config.deployment_mode {
        DeploymentMode::Local => run_local(&cmd),
        DeploymentMode::Remote => ssh_client(config, &cmd),
    };

    if config.deployment_mode == DeploymentMode::Remote {
        cleanup_remote_file(config, RemoteSide::Client, &format!("/tmp/{key}.toml"));
    }

    match result {
        Ok(out) if out.status.success() => {
            let _ = fs::write(output_path, &out.stdout);
            let parsed: Option<Value> = serde_json::from_slice(&out.stdout).ok();
            if let Some(ref value) = parsed {
                let metrics = spinr_metrics(value);
                let success_rate = validation_success_rate(metrics);
                println!(
                    "    -> rps={} p99={}ms success={:.1}%",
                    val_str(metrics, "rps"),
                    val_str(metrics, "latency_p99_ms"),
                    success_rate * 100.0
                );
                if let Err(error) = validate_spinr_metrics(metrics) {
                    return BenchRunResult {
                        value: parsed,
                        error: Some(error),
                        raw_output_path: output_path.to_path_buf(),
                    };
                }
                return BenchRunResult {
                    value: parsed,
                    error: None,
                    raw_output_path: output_path.to_path_buf(),
                };
            }

            BenchRunResult {
                value: None,
                error: Some("spinr returned non-JSON output".into()),
                raw_output_path: output_path.to_path_buf(),
            }
        }
        Ok(out) => BenchRunResult {
            value: None,
            error: Some(format!(
                "spinr benchmark failed (exit {}): {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )),
            raw_output_path: output_path.to_path_buf(),
        },
        Err(error) => BenchRunResult {
            value: None,
            error: Some(format!("failed to run spinr benchmark: {error}")),
            raw_output_path: output_path.to_path_buf(),
        },
    }
}

fn run_vegeta_bench(
    config: &CommonRunConfig,
    case: &CaseSpec,
    key: &str,
    rendered_template: &Path,
    output_path: &Path,
) -> BenchRunResult {
    let duration = case.duration_secs.unwrap_or(config.duration_secs);
    let warmup = case.warmup_secs.unwrap_or(config.warmup_secs);
    let rate = case.rate.unwrap_or(1_000);
    let workers = resolved_concurrency(case).unwrap_or(128);
    let duration_str = format!("{duration}s");
    let warmup_str = format!("{warmup}s");

    if warmup > 0 {
        let warmup_cmd = vegeta_attack_command(
            config,
            key,
            rendered_template,
            &warmup_str,
            rate,
            workers,
            false,
        );
        let _ = match config.deployment_mode {
            DeploymentMode::Local => run_local(&warmup_cmd),
            DeploymentMode::Remote => ssh_client(config, &warmup_cmd),
        };
    }

    let cmd = vegeta_attack_command(
        config,
        key,
        rendered_template,
        &duration_str,
        rate,
        workers,
        true,
    );
    let result = match config.deployment_mode {
        DeploymentMode::Local => run_local(&cmd),
        DeploymentMode::Remote => ssh_client(config, &cmd),
    };

    if config.deployment_mode == DeploymentMode::Remote {
        cleanup_remote_file(config, RemoteSide::Client, &format!("/tmp/{key}.targets"));
    }

    match result {
        Ok(out) if out.status.success() => {
            let metrics: Option<VegetaMetrics> = serde_json::from_slice(&out.stdout).ok();
            if let Some(metrics) = metrics {
                let converted = convert_vegeta_to_spinr_format(&metrics);
                let _ = fs::write(
                    output_path,
                    serde_json::to_vec_pretty(&converted).unwrap_or_default(),
                );

                println!(
                    "    -> rps={:.3} p99={:.3}ms success={:.1}%",
                    metrics.throughput,
                    metrics.latencies.p99 / 1_000_000.0,
                    metrics.success * 100.0
                );

                let error = validate_vegeta_metrics(&metrics).err();
                BenchRunResult {
                    value: Some(converted),
                    error,
                    raw_output_path: output_path.to_path_buf(),
                }
            } else {
                let _ = fs::write(output_path, &out.stdout);
                BenchRunResult {
                    value: None,
                    error: Some("vegeta returned non-JSON output".into()),
                    raw_output_path: output_path.to_path_buf(),
                }
            }
        }
        Ok(out) => BenchRunResult {
            value: None,
            error: Some(format!(
                "vegeta benchmark failed (exit {}): {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )),
            raw_output_path: output_path.to_path_buf(),
        },
        Err(error) => BenchRunResult {
            value: None,
            error: Some(format!("failed to run vegeta benchmark: {error}")),
            raw_output_path: output_path.to_path_buf(),
        },
    }
}

fn vegeta_attack_command(
    config: &CommonRunConfig,
    key: &str,
    rendered_template: &Path,
    duration_str: &str,
    rate: u32,
    workers: u32,
    with_report: bool,
) -> String {
    let target_path = match config.deployment_mode {
        DeploymentMode::Local => rendered_template.display().to_string(),
        DeploymentMode::Remote => {
            let remote_target = format!("/tmp/{key}.targets");
            scp_to_client(config, rendered_template, &remote_target);
            remote_target
        }
    };

    let attack_cmd = format!(
        "docker run --rm --network host -v {target_path}:/targets.txt {DEFAULT_VEGETA_IMAGE} attack -targets=/targets.txt -duration={duration_str} -rate={rate}/s -workers={workers} -max-workers={workers}"
    );

    if with_report {
        format!("{attack_cmd} | docker run --rm -i {DEFAULT_VEGETA_IMAGE} report -type=json")
    } else {
        format!("{attack_cmd} >/dev/null")
    }
}

fn write_canonical_report(
    mode: RunMode,
    config: &CommonRunConfig,
    suite: &SuiteSpec,
    results_dir: &Path,
    started_at_utc: &str,
    completed_at_utc: &str,
    case_records: &[CaseExecutionRecord],
) -> Result<(), String> {
    let mut cases = Vec::with_capacity(case_records.len());
    for record in case_records {
        let mut results = Vec::with_capacity(record.results.len());
        for run in &record.results {
            results.push(ImplementationResult {
                implementation_id: run.variant.implementation.id.clone(),
                compare_side: run.variant.compare_side,
                started_at_utc: run.started_at_utc.clone(),
                completed_at_utc: run.completed_at_utc.clone(),
                image: ImageDescriptor {
                    tag: run.variant.implementation.image.clone(),
                    id: run.image_id.clone(),
                },
                labels: ImplementationLabels {
                    framework: run.variant.implementation.framework_label().to_string(),
                    backend: run.variant.implementation.backend_label().to_string(),
                    profile: run.variant.implementation.profile_label().to_string(),
                },
                metrics: canonical_metrics(&run.metrics),
                artifacts: ResultArtifacts {
                    loadgen_raw: relative_display(results_dir, &run.raw_metrics_path),
                },
                os: None,
                perf: None,
            });
        }

        cases.push(CaseReport {
            id: record.case.id.clone(),
            generator: record.case.generator,
            template: TemplateFiles {
                source: record
                    .case
                    .resolved_template_path(&config.suite_path)
                    .display()
                    .to_string(),
                rendered: relative_display(results_dir, &record.rendered_template),
            },
            load: LoadProfile {
                concurrency: resolved_concurrency(&record.case),
                rate: record.case.rate,
                duration_secs: record.case.duration_secs.unwrap_or(config.duration_secs),
                warmup_secs: record.case.warmup_secs.unwrap_or(config.warmup_secs),
            },
            results,
        });
    }

    let report = RunReport {
        schema_version: RUN_SCHEMA_VERSION,
        run_mode: mode,
        deployment_mode: config.deployment_mode,
        suite: SuiteDescriptor {
            name: suite.name.clone(),
            path: config.suite_path.display().to_string(),
        },
        targets: TargetDescriptor {
            server_host: config
                .server_ssh
                .clone()
                .unwrap_or_else(|| "localhost".into()),
            client_host: config
                .client_ssh
                .clone()
                .unwrap_or_else(|| "localhost".into()),
            server_private_ip: config
                .server_private_ip
                .clone()
                .unwrap_or_else(|| "127.0.0.1".into()),
            port: config.port,
        },
        defaults: RunDefaults {
            duration_secs: config.duration_secs,
            warmup_secs: config.warmup_secs,
        },
        started_at_utc: started_at_utc.to_string(),
        completed_at_utc: completed_at_utc.to_string(),
        git: GitDescriptor {
            sha: git_sha(),
            dirty: git_dirty(),
        },
        cases,
    };

    let path = results_dir.join("run.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&report).unwrap_or_default(),
    )
    .map_err(|e| format!("failed to write canonical report {}: {e}", path.display()))
}

fn canonical_metrics(value: &Value) -> BenchmarkMetrics {
    let metrics = spinr_metrics(value);
    BenchmarkMetrics {
        rps: metric_f64(metrics, "rps"),
        success_rate: validation_success_rate(metrics),
        status_codes: status_code_map(metrics),
        latency_ms: LatencyMetrics {
            p50: metric_f64(metrics, "latency_p50_ms"),
            p95: metric_f64(metrics, "latency_p95_ms"),
            p99: metric_f64(metrics, "latency_p99_ms"),
            p999: metric_f64(metrics, "latency_p999_ms"),
            max: metric_f64(metrics, "latency_max_ms"),
        },
    }
}

fn image_id(config: &CommonRunConfig, image: &str) -> Option<String> {
    let cmd = format!("docker image inspect -f '{{{{.Id}}}}' {image}");
    let output = match config.deployment_mode {
        DeploymentMode::Local => run_local(&cmd).ok()?,
        DeploymentMode::Remote => ssh_server(config, &cmd).ok()?,
    };
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn relative_display(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn resolved_concurrency(case: &CaseSpec) -> Option<u32> {
    case.concurrency.or_else(|| {
        case.context
            .get("connections")
            .or_else(|| case.context.get("concurrency"))
            .and_then(|value| value.as_integer())
            .map(|value| value as u32)
    })
}

fn run_key(variant_label: &str, case_id: &str) -> String {
    format!(
        "{}_{}",
        sanitize_label(variant_label),
        sanitize_label(case_id)
    )
}

fn container_name(label: &str) -> String {
    format!("bench-{}", sanitize_label(label))
}

fn sanitize_label(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect()
}

fn spinr_metrics<'a>(value: &'a Value) -> &'a Value {
    value.pointer("/scenarios/0/metrics").unwrap_or(value)
}

fn metric_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn metric_f64(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or_default()
}

fn status_code_map(metrics: &Value) -> BTreeMap<String, u64> {
    metrics
        .get("status_codes")
        .and_then(Value::as_object)
        .map(|codes| {
            codes
                .iter()
                .filter_map(|(status, count)| count.as_u64().map(|count| (status.clone(), count)))
                .collect()
        })
        .unwrap_or_default()
}

fn validation_success_rate(metrics: &Value) -> f64 {
    let successful = metric_u64(metrics, "successful_requests").unwrap_or_default();
    let failed = metric_u64(metrics, "failed_requests").unwrap_or_default();
    let total = metric_u64(metrics, "total_requests").unwrap_or(successful + failed);
    if total > 0 {
        successful as f64 / total as f64
    } else if failed == 0 {
        1.0
    } else {
        0.0
    }
}

fn validate_spinr_metrics(metrics: &Value) -> Result<(), String> {
    let successful = metric_u64(metrics, "successful_requests").unwrap_or_default();
    let failed = metric_u64(metrics, "failed_requests").unwrap_or_default();
    let total = metric_u64(metrics, "total_requests").unwrap_or(successful + failed);
    let success_rate = validation_success_rate(metrics);

    if successful == 0 {
        return Err(format!(
            "benchmark produced no successful requests (status_codes={})",
            format_status_codes(metrics)
        ));
    }
    if failed > 0 {
        return Err(format!(
            "benchmark reported {failed} failed requests out of {total} (success={:.1}%, status_codes={})",
            success_rate * 100.0,
            format_status_codes(metrics)
        ));
    }

    Ok(())
}

fn validate_vegeta_metrics(metrics: &VegetaMetrics) -> Result<(), String> {
    if metrics.success <= 0.0 {
        return Err(format!(
            "benchmark produced no successful requests (status_codes={})",
            format_status_code_map(&metrics.status_codes)
        ));
    }
    if metrics.success < 1.0 {
        return Err(format!(
            "benchmark reported failed requests (success={:.1}%, status_codes={})",
            metrics.success * 100.0,
            format_status_code_map(&metrics.status_codes)
        ));
    }

    Ok(())
}

fn format_status_codes(metrics: &Value) -> String {
    metrics
        .get("status_codes")
        .map(Value::to_string)
        .unwrap_or_else(|| "-".into())
}

fn format_status_code_map(codes: &BTreeMap<String, u64>) -> String {
    if codes.is_empty() {
        return "-".into();
    }
    codes
        .iter()
        .map(|(status, count)| format!("{status}:{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn convert_vegeta_to_spinr_format(metrics: &VegetaMetrics) -> Value {
    let ns_to_ms = |ns: f64| ns / 1_000_000.0;
    serde_json::json!({
        "scenarios": [{
            "metrics": {
                "rps": metrics.throughput,
                "latency_mean_ms": ns_to_ms(metrics.latencies.mean),
                "latency_p50_ms": ns_to_ms(metrics.latencies.p50),
                "latency_p95_ms": ns_to_ms(metrics.latencies.p95),
                "latency_p99_ms": ns_to_ms(metrics.latencies.p99),
                "latency_p999_ms": ns_to_ms(metrics.latencies.max),
                "latency_max_ms": ns_to_ms(metrics.latencies.max),
                "success_rate": metrics.success,
                "status_codes": metrics.status_codes
            }
        }]
    })
}

fn val_str(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::Number(number)) => {
            if let Some(float) = number.as_f64() {
                if float == float.floor() && float.abs() < 1e15 {
                    format!("{}", float as i64)
                } else {
                    format!("{float:.3}")
                }
            } else {
                number.to_string()
            }
        }
        Some(other) => other.to_string(),
        None => "-".into(),
    }
}

#[derive(Clone, Copy)]
enum RemoteSide {
    Client,
}

fn run_local(cmd: &str) -> std::io::Result<Output> {
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

fn ssh_run(user: &str, host: &str, remote_cmd: &str) -> std::io::Result<Output> {
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

fn ssh_server(config: &CommonRunConfig, remote_cmd: &str) -> std::io::Result<Output> {
    ssh_run(
        &config.ssh_user,
        config.server_ssh.as_deref().unwrap_or_default(),
        remote_cmd,
    )
}

fn ssh_client(config: &CommonRunConfig, remote_cmd: &str) -> std::io::Result<Output> {
    ssh_run(
        &config.ssh_user,
        config.client_ssh.as_deref().unwrap_or_default(),
        remote_cmd,
    )
}

fn ssh_side(
    config: &CommonRunConfig,
    side: RemoteSide,
    remote_cmd: &str,
) -> std::io::Result<Output> {
    match side {
        RemoteSide::Client => ssh_client(config, remote_cmd),
    }
}

fn scp_to_remote(config: &CommonRunConfig, host: &str, local_path: &Path, remote_path: &str) {
    let out = Command::new("scp")
        .arg("-o")
        .arg("StrictHostKeyChecking=no")
        .arg(local_path)
        .arg(format!("{}@{host}:{remote_path}", config.ssh_user))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match out {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            eprintln!(
                "    warning: scp to {} failed: {}",
                remote_path,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Err(error) => eprintln!("    warning: scp to {} failed: {error}", remote_path),
    }
}

fn scp_to_client(config: &CommonRunConfig, local_path: &Path, remote_path: &str) {
    if let Some(host) = config.client_ssh.as_deref() {
        scp_to_remote(config, host, local_path, remote_path);
    }
}

fn cleanup_remote_file(config: &CommonRunConfig, side: RemoteSide, remote_path: &str) {
    let _ = ssh_side(config, side, &format!("rm -f {remote_path}"));
}

fn git_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn git_dirty() -> bool {
    Command::new("git")
        .args(["status", "--short"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .is_some_and(|output| output.status.success() && !output.stdout.is_empty())
}

fn chrono_lite_utc() -> String {
    match Command::new("date")
        .args(["-u", "+%Y-%m-%d %H:%M:%S UTC"])
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(_) => "unknown".into(),
    }
}

fn timestamp_slug() -> String {
    match Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H-%M-%SZ"])
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(_) => "unknown".into(),
    }
}
