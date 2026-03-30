use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::spec::{DeploymentMode, LoadGeneratorKind, RunMode};

pub const RUN_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunReport {
    pub schema_version: u32,
    pub run_mode: RunMode,
    pub deployment_mode: DeploymentMode,
    pub suite: SuiteDescriptor,
    pub targets: TargetDescriptor,
    pub defaults: RunDefaults,
    pub started_at_utc: String,
    pub completed_at_utc: String,
    pub git: GitDescriptor,
    pub cases: Vec<CaseReport>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SuiteDescriptor {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TargetDescriptor {
    pub server_host: String,
    pub client_host: String,
    pub server_private_ip: String,
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunDefaults {
    pub duration_secs: u32,
    pub warmup_secs: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GitDescriptor {
    pub sha: String,
    pub dirty: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CaseReport {
    pub id: String,
    pub generator: LoadGeneratorKind,
    pub template: TemplateFiles,
    pub load: LoadProfile,
    pub results: Vec<ImplementationResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TemplateFiles {
    pub source: String,
    pub rendered: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LoadProfile {
    pub concurrency: Option<u32>,
    pub rate: Option<u32>,
    pub duration_secs: u32,
    pub warmup_secs: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImplementationResult {
    pub implementation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare_side: Option<CompareSide>,
    pub started_at_utc: String,
    pub completed_at_utc: String,
    pub image: ImageDescriptor,
    pub labels: ImplementationLabels,
    pub metrics: BenchmarkMetrics,
    pub artifacts: ResultArtifacts,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<OsData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub perf: Option<PerfData>,
}

impl ImplementationResult {
    pub fn display_label(&self) -> &str {
        &self.implementation_id
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompareSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImageDescriptor {
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ImplementationLabels {
    pub framework: String,
    pub backend: String,
    pub profile: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BenchmarkMetrics {
    pub rps: f64,
    pub success_rate: f64,
    pub status_codes: BTreeMap<String, u64>,
    pub latency_ms: LatencyMetrics,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LatencyMetrics {
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub p999: f64,
    pub max: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResultArtifacts {
    pub loadgen_raw: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OsData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<OsSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<OsSeries>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<OsArtifacts>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OsSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_cpu: Option<CpuSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_cpu: Option<CpuSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_net: Option<NetSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_net: Option<NetSummary>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CpuSummary {
    pub user: f64,
    pub system: f64,
    pub iowait: f64,
    pub idle: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NetSummary {
    pub rx_kb_s: f64,
    pub tx_kb_s: f64,
    pub retrans_s: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OsSeries {
    #[serde(default)]
    pub server_cpu_busy_pct: Vec<f64>,
    #[serde(default)]
    pub server_net_tx_mb_s: Vec<f64>,
    #[serde(default)]
    pub server_run_queue: Vec<f64>,
    #[serde(default)]
    pub server_context_switches_s: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OsArtifacts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_cpu_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_cpu_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_net_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_net_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_vmstat_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PerfData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<PerfSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<PerfArtifacts>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PerfSummary {
    #[serde(default)]
    pub hotspots: Vec<PerfHotspot>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PerfHotspot {
    pub pct: f64,
    pub shared_object: String,
    pub symbol: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PerfArtifacts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folded_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flamegraph_svg: Option<String>,
}
