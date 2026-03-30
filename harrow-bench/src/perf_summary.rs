use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::harness::schema::{
    BenchmarkMetrics, CaseReport, CompareSide, CpuSummary, ImplementationResult, NetSummary,
    OsSeries, RunReport,
};
use crate::harness::spec::RunMode;

const CANVAS: &str = "#fafaf9";
const SURFACE: &str = "#ffffff";
const BORDER_LIGHT: &str = "#e7e5e4";
const TEXT_PRIMARY: &str = "#1c1917";
const TEXT_SECONDARY: &str = "#57534e";
const TEXT_MUTED: &str = "#a8a29e";
const PRIMARY_COLOR: &str = "#ea580c";
const SECONDARY_COLOR: &str = "#0f766e";
const TERTIARY_COLOR: &str = "#1d4ed8";
const QUATERNARY_COLOR: &str = "#b45309";
const PRIMARY_FILL: &str = "#fff7ed";
const SECONDARY_FILL: &str = "#ecfdf5";
const TERTIARY_FILL: &str = "#eff6ff";
const QUATERNARY_FILL: &str = "#fef3c7";

#[derive(Clone, Copy)]
struct PanelRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

struct SeriesPanel<'a> {
    title: &'a str,
    unit: &'a str,
    primary_label: &'a str,
    secondary_label: Option<&'a str>,
    primary: &'a [f64],
    secondary: &'a [f64],
}

pub fn render_results_dir(results_dir: &Path) -> io::Result<()> {
    let report = load_run_report(results_dir)?;
    let flamegraphs = prepare_local_flamegraphs(results_dir, &report)?;
    generate_case_svgs(results_dir, &report)?;
    generate_telemetry_svgs(results_dir, &report)?;

    fs::write(results_dir.join("summary.svg"), render_summary_svg(&report))?;
    fs::write(
        results_dir.join("summary.md"),
        render_markdown(results_dir, &report, &flamegraphs),
    )?;
    Ok(())
}

fn load_run_report(results_dir: &Path) -> io::Result<RunReport> {
    let path = results_dir.join("run.json");
    let bytes = fs::read(&path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::other(format!("failed to parse {}: {error}", path.display())))
}

fn render_markdown(
    results_dir: &Path,
    report: &RunReport,
    flamegraphs: &BTreeMap<String, String>,
) -> String {
    let mut out = String::new();
    let run_label = results_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("benchmark-run");

    writeln!(&mut out, "# Benchmark Results").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "![Run Dashboard](summary.svg)").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(&mut out, "Run: `{run_label}`").unwrap();
    writeln!(&mut out, "Suite: `{}`", report.suite.name).unwrap();
    writeln!(&mut out, "Mode: `{}`", report.run_mode.as_str()).unwrap();
    writeln!(
        &mut out,
        "Deployment: `{}`",
        report.deployment_mode.as_str()
    )
    .unwrap();
    writeln!(
        &mut out,
        "Server: `{}` (`{}:{}`)",
        report.targets.server_host, report.targets.server_private_ip, report.targets.port
    )
    .unwrap();
    writeln!(&mut out, "Client: `{}`", report.targets.client_host).unwrap();
    writeln!(
        &mut out,
        "Defaults: {}s duration / {}s warmup",
        report.defaults.duration_secs, report.defaults.warmup_secs
    )
    .unwrap();
    writeln!(
        &mut out,
        "Git: `{}`{}",
        report.git.sha,
        if report.git.dirty { " (dirty)" } else { "" }
    )
    .unwrap();
    writeln!(&mut out, "Completed: {}", report.completed_at_utc).unwrap();
    writeln!(&mut out).unwrap();

    writeln!(&mut out, "## Runs").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(
        &mut out,
        "| Case | Implementation | Generator | Concurrency | Rate | Success % | Codes | RPS | p50 (ms) | p95 (ms) | p99 (ms) | p999 (ms) |"
    )
    .unwrap();
    writeln!(
        &mut out,
        "|------|----------------|-----------|-------------|------|-----------|-------|-----|----------|----------|----------|-----------|"
    )
    .unwrap();
    for case in &report.cases {
        for result in sorted_results(case) {
            writeln!(
                &mut out,
                "| {} | {} | {} | {} | {} | {:.1}% | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |",
                case.id,
                result.display_label(),
                case.generator.as_str(),
                format_optional_u32(case.load.concurrency),
                format_optional_u32(case.load.rate),
                result.metrics.success_rate * 100.0,
                format_status_codes(&result.metrics),
                result.metrics.rps,
                result.metrics.latency_ms.p50,
                result.metrics.latency_ms.p95,
                result.metrics.latency_ms.p99,
                result.metrics.latency_ms.p999
            )
            .unwrap();
        }
    }
    writeln!(&mut out).unwrap();

    if report.run_mode == RunMode::Compare {
        writeln!(&mut out, "## Comparison").unwrap();
        writeln!(&mut out).unwrap();
        writeln!(
            &mut out,
            "| Case | Left | Right | RPS Delta % | Left p99 (ms) | Right p99 (ms) |"
        )
        .unwrap();
        writeln!(
            &mut out,
            "|------|------|-------|-------------|---------------|----------------|"
        )
        .unwrap();
        for case in &report.cases {
            let results = sorted_results(case);
            if results.len() != 2 {
                continue;
            }
            let left = results[0];
            let right = results[1];
            writeln!(
                &mut out,
                "| {} | {} | {} | {:+.2}% | {:.3} | {:.3} |",
                case.id,
                left.display_label(),
                right.display_label(),
                pct_delta(left.metrics.rps, right.metrics.rps),
                left.metrics.latency_ms.p99,
                right.metrics.latency_ms.p99
            )
            .unwrap();
        }
        writeln!(&mut out).unwrap();
    }

    writeln!(&mut out, "## Case Graphs").unwrap();
    writeln!(&mut out).unwrap();
    for case in &report.cases {
        writeln!(&mut out, "### {}", case.id).unwrap();
        writeln!(&mut out).unwrap();
        writeln!(
            &mut out,
            "![{} graph](./{})",
            case.id,
            case_svg_filename(&case.id)
        )
        .unwrap();
        writeln!(&mut out).unwrap();
    }

    let telemetry_cases: Vec<&CaseReport> = report
        .cases
        .iter()
        .filter(|case| results_dir.join(telemetry_svg_filename(&case.id)).exists())
        .collect();
    if !telemetry_cases.is_empty() {
        writeln!(&mut out, "## OS Telemetry").unwrap();
        writeln!(&mut out).unwrap();
        for case in telemetry_cases {
            writeln!(&mut out, "### {}", case.id).unwrap();
            writeln!(&mut out).unwrap();
            writeln!(
                &mut out,
                "![{} telemetry](./{})",
                case.id,
                telemetry_svg_filename(&case.id)
            )
            .unwrap();
            writeln!(&mut out).unwrap();
        }
    }

    writeln!(&mut out, "## Artifacts").unwrap();
    writeln!(&mut out).unwrap();
    writeln!(
        &mut out,
        "| Case | Implementation | Rendered Template | Loadgen Raw | Perf Report | Perf Script | Flamegraph | Server CPU | Server Net | Client CPU | Client Net | VMstat |"
    )
    .unwrap();
    writeln!(
        &mut out,
        "|------|----------------|-------------------|-------------|-------------|-------------|------------|------------|------------|------------|------------|--------|"
    )
    .unwrap();
    for case in &report.cases {
        for result in sorted_results(case) {
            let key = result_key(case, result);
            let perf_artifacts = result
                .perf
                .as_ref()
                .and_then(|perf| perf.artifacts.as_ref());
            let os_artifacts = result.os.as_ref().and_then(|os| os.artifacts.as_ref());
            writeln!(
                &mut out,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                case.id,
                result.display_label(),
                markdown_link(&case.template.rendered, "rendered"),
                markdown_link(&result.artifacts.loadgen_raw, "json"),
                markdown_link_opt(
                    perf_artifacts.and_then(|artifacts| artifacts.report_path.as_deref()),
                    "perf-report"
                ),
                markdown_link_opt(
                    perf_artifacts.and_then(|artifacts| artifacts.script_path.as_deref()),
                    "perf-script"
                ),
                markdown_link_opt(flamegraphs.get(&key).map(String::as_str), "flamegraph"),
                markdown_link_opt(
                    os_artifacts.and_then(|artifacts| artifacts.server_cpu_path.as_deref()),
                    "server cpu"
                ),
                markdown_link_opt(
                    os_artifacts.and_then(|artifacts| artifacts.server_net_path.as_deref()),
                    "server net"
                ),
                markdown_link_opt(
                    os_artifacts.and_then(|artifacts| artifacts.client_cpu_path.as_deref()),
                    "client cpu"
                ),
                markdown_link_opt(
                    os_artifacts.and_then(|artifacts| artifacts.client_net_path.as_deref()),
                    "client net"
                ),
                markdown_link_opt(
                    os_artifacts.and_then(|artifacts| artifacts.server_vmstat_path.as_deref()),
                    "vmstat"
                ),
            )
            .unwrap();
        }
    }

    if !flamegraphs.is_empty() {
        writeln!(&mut out).unwrap();
        writeln!(&mut out, "## Flamegraphs").unwrap();
        writeln!(&mut out).unwrap();
        for case in &report.cases {
            for result in sorted_results(case) {
                let key = result_key(case, result);
                let Some(path) = flamegraphs.get(&key) else {
                    continue;
                };
                writeln!(&mut out, "### {} / {}", case.id, result.display_label()).unwrap();
                writeln!(&mut out).unwrap();
                writeln!(
                    &mut out,
                    "![{} flamegraph](./{})",
                    result.display_label(),
                    path
                )
                .unwrap();
                writeln!(&mut out).unwrap();
            }
        }
    }

    out
}

fn render_summary_svg(report: &RunReport) -> String {
    let width = 1400.0;
    let max_results = report
        .cases
        .iter()
        .map(|case| case.results.len())
        .max()
        .unwrap_or(1)
        .max(1);
    let slot_height = 46.0 + max_results as f64 * 22.0;
    let panel_height = 100.0 + report.cases.len() as f64 * slot_height;
    let height = 180.0 + panel_height;
    let panel_gap = 28.0;
    let panel_w = (width - 44.0 * 2.0 - panel_gap) / 2.0;
    let left_x = 44.0;
    let right_x = left_x + panel_w + panel_gap;
    let panel_y = 118.0;

    let max_rps = report
        .cases
        .iter()
        .flat_map(|case| case.results.iter().map(|result| result.metrics.rps))
        .fold(0.0, f64::max)
        .max(1.0);
    let max_p99 = report
        .cases
        .iter()
        .flat_map(|case| {
            case.results
                .iter()
                .map(|result| result.metrics.latency_ms.p99)
        })
        .fold(0.0, f64::max)
        .max(1.0);

    let mut svg = String::new();
    writeln!(
        &mut svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" fill="none">"##
    )
    .unwrap();
    writeln!(
        &mut svg,
        r##"<rect x="0" y="0" width="{width:.0}" height="{height:.0}" fill="{CANVAS}"/>"##
    )
    .unwrap();
    svg.push_str(&mono_text(
        44.0,
        50.0,
        20,
        700,
        TEXT_PRIMARY,
        &format!("benchmark dashboard :: {}", report.suite.name),
    ));
    svg.push_str(&ui_text(
        44.0,
        80.0,
        14,
        500,
        TEXT_SECONDARY,
        &format!(
            "{} · server {} ({}) · client {} · {}s / {}s",
            report.run_mode.as_str(),
            report.targets.server_host,
            report.targets.server_private_ip,
            report.targets.client_host,
            report.defaults.duration_secs,
            report.defaults.warmup_secs
        ),
    ));

    svg.push_str(&panel_card(
        left_x,
        panel_y,
        panel_w,
        panel_height,
        PRIMARY_COLOR,
    ));
    svg.push_str(&panel_card(
        right_x,
        panel_y,
        panel_w,
        panel_height,
        PRIMARY_COLOR,
    ));
    svg.push_str(&mono_text(
        left_x + 24.0,
        panel_y + 34.0,
        18,
        700,
        PRIMARY_COLOR,
        "throughput",
    ));
    svg.push_str(&mono_text(
        right_x + 24.0,
        panel_y + 34.0,
        18,
        700,
        PRIMARY_COLOR,
        "p99 latency",
    ));

    for (case_idx, case) in report.cases.iter().enumerate() {
        let base_y = panel_y + 76.0 + case_idx as f64 * slot_height;
        let sorted = sorted_results(case);
        svg.push_str(&mono_text(
            left_x + 24.0,
            base_y,
            13,
            700,
            TEXT_PRIMARY,
            &case.id,
        ));
        svg.push_str(&mono_text(
            right_x + 24.0,
            base_y,
            13,
            700,
            TEXT_PRIMARY,
            &case.id,
        ));

        for (idx, result) in sorted.iter().enumerate() {
            let (stroke, fill) = accent(idx);
            let y = base_y + 12.0 + idx as f64 * 22.0;
            let throughput_w = ((result.metrics.rps / max_rps) * (panel_w - 220.0)).max(4.0);
            let p99_w = ((result.metrics.latency_ms.p99 / max_p99) * (panel_w - 220.0)).max(4.0);

            svg.push_str(&mono_text(
                left_x + 24.0,
                y + 12.0,
                12,
                600,
                stroke,
                result.display_label(),
            ));
            svg.push_str(&mono_text(
                right_x + 24.0,
                y + 12.0,
                12,
                600,
                stroke,
                result.display_label(),
            ));
            svg.push_str(&metric_bar(
                left_x + 132.0,
                y,
                throughput_w,
                14.0,
                stroke,
                fill,
            ));
            svg.push_str(&metric_bar(right_x + 132.0, y, p99_w, 14.0, stroke, fill));
            svg.push_str(&ui_text(
                left_x + 140.0 + (panel_w - 220.0),
                y + 12.0,
                10,
                500,
                TEXT_SECONDARY,
                &format!("{:.0} rps", result.metrics.rps),
            ));
            svg.push_str(&ui_text(
                right_x + 140.0 + (panel_w - 220.0),
                y + 12.0,
                10,
                500,
                TEXT_SECONDARY,
                &format!("{:.3} ms", result.metrics.latency_ms.p99),
            ));
        }

        if report.run_mode == RunMode::Compare && sorted.len() == 2 {
            svg.push_str(&ui_text(
                left_x + 24.0,
                base_y + 20.0 + sorted.len() as f64 * 22.0,
                11,
                500,
                TEXT_SECONDARY,
                &format!(
                    "delta {} vs {}: {:+.2}%",
                    sorted[0].display_label(),
                    sorted[1].display_label(),
                    pct_delta(sorted[0].metrics.rps, sorted[1].metrics.rps)
                ),
            ));
        }
    }

    svg.push_str("</svg>");
    svg
}

fn generate_case_svgs(results_dir: &Path, report: &RunReport) -> io::Result<()> {
    for case in &report.cases {
        fs::write(
            results_dir.join(case_svg_filename(&case.id)),
            render_case_svg(case),
        )?;
    }
    Ok(())
}

fn render_case_svg(case: &CaseReport) -> String {
    let width = 1400.0;
    let results = sorted_results(case);
    let panel_gap = 28.0;
    let panel_w = (width - 44.0 * 2.0 - panel_gap) / 2.0;
    let top_y = 132.0;
    let top_h = 200.0;
    let card_h = 170.0;
    let card_gap = 20.0;
    let rows = results.len().div_ceil(2).max(1) as f64;
    let height = 160.0 + top_h + 32.0 + rows * (card_h + card_gap);

    let max_rps = results
        .iter()
        .map(|result| result.metrics.rps)
        .fold(0.0, f64::max)
        .max(1.0);
    let max_latency = results
        .iter()
        .map(|result| result.metrics.latency_ms.p99)
        .fold(0.0, f64::max)
        .max(1.0);

    let mut svg = String::new();
    writeln!(
        &mut svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" fill="none">"##
    )
    .unwrap();
    writeln!(
        &mut svg,
        r##"<rect x="0" y="0" width="{width:.0}" height="{height:.0}" fill="{CANVAS}"/>"##
    )
    .unwrap();
    svg.push_str(&mono_text(
        44.0,
        52.0,
        20,
        700,
        TEXT_PRIMARY,
        &format!("case :: {}", case.id),
    ));
    svg.push_str(&ui_text(
        44.0,
        82.0,
        14,
        500,
        TEXT_SECONDARY,
        &format!(
            "{} · {} concurrency · {} rate · {}s / {}s",
            case.generator.as_str(),
            format_optional_u32(case.load.concurrency),
            format_optional_u32(case.load.rate),
            case.load.duration_secs,
            case.load.warmup_secs
        ),
    ));

    let left_x = 44.0;
    let right_x = left_x + panel_w + panel_gap;
    svg.push_str(&panel_card(left_x, top_y, panel_w, top_h, PRIMARY_COLOR));
    svg.push_str(&panel_card(right_x, top_y, panel_w, top_h, PRIMARY_COLOR));
    svg.push_str(&mono_text(
        left_x + 24.0,
        top_y + 34.0,
        18,
        700,
        PRIMARY_COLOR,
        "throughput",
    ));
    svg.push_str(&mono_text(
        right_x + 24.0,
        top_y + 34.0,
        18,
        700,
        PRIMARY_COLOR,
        "latency",
    ));

    for (idx, result) in results.iter().enumerate() {
        let (stroke, fill) = accent(idx);
        let y = top_y + 60.0 + idx as f64 * 30.0;
        let throughput_w = ((result.metrics.rps / max_rps) * (panel_w - 240.0)).max(4.0);
        let latency_w =
            ((result.metrics.latency_ms.p99 / max_latency) * (panel_w - 240.0)).max(4.0);

        svg.push_str(&mono_text(
            left_x + 24.0,
            y + 13.0,
            12,
            600,
            stroke,
            result.display_label(),
        ));
        svg.push_str(&mono_text(
            right_x + 24.0,
            y + 13.0,
            12,
            600,
            stroke,
            result.display_label(),
        ));
        svg.push_str(&metric_bar(
            left_x + 132.0,
            y,
            throughput_w,
            16.0,
            stroke,
            fill,
        ));
        svg.push_str(&metric_bar(
            right_x + 132.0,
            y,
            latency_w,
            16.0,
            stroke,
            fill,
        ));
        svg.push_str(&ui_text(
            left_x + 140.0 + (panel_w - 240.0),
            y + 13.0,
            10,
            500,
            TEXT_SECONDARY,
            &format!("{:.0} rps", result.metrics.rps),
        ));
        svg.push_str(&ui_text(
            right_x + 140.0 + (panel_w - 240.0),
            y + 13.0,
            10,
            500,
            TEXT_SECONDARY,
            &format!("p99 {:.3} ms", result.metrics.latency_ms.p99),
        ));
    }

    let cards_y = top_y + top_h + 32.0;
    for (idx, result) in results.iter().enumerate() {
        let col = (idx % 2) as f64;
        let row = (idx / 2) as f64;
        let x = 44.0 + col * (panel_w + panel_gap);
        let y = cards_y + row * (card_h + card_gap);
        let (accent_color, _) = accent(idx);
        svg.push_str(&panel_card(x, y, panel_w, card_h, accent_color));
        svg.push_str(&mono_text(
            x + 24.0,
            y + 34.0,
            18,
            700,
            accent_color,
            result.display_label(),
        ));
        svg.push_str(&ui_text(
            x + 24.0,
            y + 58.0,
            10,
            500,
            TEXT_SECONDARY,
            &format!(
                "{} / {} / {}",
                result.labels.framework, result.labels.backend, result.labels.profile
            ),
        ));
        svg.push_str(&metric_chip(
            x + 24.0,
            y + 78.0,
            "RPS",
            &format!("{:.0}", result.metrics.rps),
            accent_color,
        ));
        svg.push_str(&metric_chip(
            x + 168.0,
            y + 78.0,
            "p50",
            &format!("{:.3} ms", result.metrics.latency_ms.p50),
            accent_color,
        ));
        svg.push_str(&metric_chip(
            x + 312.0,
            y + 78.0,
            "p99",
            &format!("{:.3} ms", result.metrics.latency_ms.p99),
            accent_color,
        ));
        svg.push_str(&ui_text(
            x + 24.0,
            y + 132.0,
            10,
            500,
            TEXT_SECONDARY,
            &format!(
                "success {:.1}% · codes {}",
                result.metrics.success_rate * 100.0,
                format_status_codes(&result.metrics)
            ),
        ));
        svg.push_str(&ui_text(
            x + 24.0,
            y + 154.0,
            10,
            500,
            TEXT_SECONDARY,
            &format!(
                "perf {}",
                top_perf_label(result).unwrap_or_else(|| "no perf summary recorded".to_string())
            ),
        ));
    }

    svg.push_str("</svg>");
    svg
}

fn generate_telemetry_svgs(results_dir: &Path, report: &RunReport) -> io::Result<()> {
    for case in &report.cases {
        if !case_has_os_series(case) {
            continue;
        }
        fs::write(
            results_dir.join(telemetry_svg_filename(&case.id)),
            render_telemetry_svg(case),
        )?;
    }
    Ok(())
}

fn render_telemetry_svg(case: &CaseReport) -> String {
    let results = sorted_results(case);
    let primary = results.first().copied();
    let secondary = results.get(1).copied();
    let width = 1400.0;
    let height = 860.0;
    let panel_gap = 28.0;
    let panel_w = (width - 44.0 * 2.0 - panel_gap) / 2.0;
    let panel_h = 260.0;
    let top_y = 144.0;
    let left_x = 44.0;
    let right_x = left_x + panel_w + panel_gap;
    let bottom_y = top_y + panel_h + panel_gap;

    let mut svg = String::new();
    writeln!(
        &mut svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}" fill="none">"##
    )
    .unwrap();
    writeln!(
        &mut svg,
        r##"<rect x="0" y="0" width="{width:.0}" height="{height:.0}" fill="{CANVAS}"/>"##
    )
    .unwrap();
    svg.push_str(&mono_text(
        44.0,
        52.0,
        20,
        700,
        TEXT_PRIMARY,
        &format!("server telemetry :: {}", case.id),
    ));
    svg.push_str(&ui_text(
        44.0,
        82.0,
        14,
        500,
        TEXT_SECONDARY,
        "Optional OS series recorded in run.json",
    ));

    if let Some(result) = primary {
        svg.push_str(&legend_chip(
            44.0,
            104.0,
            PRIMARY_COLOR,
            PRIMARY_FILL,
            result.display_label(),
        ));
    }
    if let Some(result) = secondary {
        svg.push_str(&legend_chip(
            184.0,
            104.0,
            SECONDARY_COLOR,
            SECONDARY_FILL,
            result.display_label(),
        ));
    }

    svg.push_str(&render_series_panel(
        PanelRect {
            x: left_x,
            y: top_y,
            w: panel_w,
            h: panel_h,
        },
        SeriesPanel {
            title: "Server CPU Busy %",
            unit: "%",
            primary_label: primary.map_or("primary", |result| result.display_label()),
            secondary_label: secondary.map(|result| result.display_label()),
            primary: primary_series(primary, |series| &series.server_cpu_busy_pct),
            secondary: secondary_series(secondary, |series| &series.server_cpu_busy_pct),
        },
    ));
    svg.push_str(&render_series_panel(
        PanelRect {
            x: right_x,
            y: top_y,
            w: panel_w,
            h: panel_h,
        },
        SeriesPanel {
            title: "Server Net TX",
            unit: "MB/s",
            primary_label: primary.map_or("primary", |result| result.display_label()),
            secondary_label: secondary.map(|result| result.display_label()),
            primary: primary_series(primary, |series| &series.server_net_tx_mb_s),
            secondary: secondary_series(secondary, |series| &series.server_net_tx_mb_s),
        },
    ));
    svg.push_str(&render_series_panel(
        PanelRect {
            x: left_x,
            y: bottom_y,
            w: panel_w,
            h: panel_h,
        },
        SeriesPanel {
            title: "VMstat Runnable (r)",
            unit: "threads",
            primary_label: primary.map_or("primary", |result| result.display_label()),
            secondary_label: secondary.map(|result| result.display_label()),
            primary: primary_series(primary, |series| &series.server_run_queue),
            secondary: secondary_series(secondary, |series| &series.server_run_queue),
        },
    ));
    svg.push_str(&render_series_panel(
        PanelRect {
            x: right_x,
            y: bottom_y,
            w: panel_w,
            h: panel_h,
        },
        SeriesPanel {
            title: "VMstat Context Switches",
            unit: "/s",
            primary_label: primary.map_or("primary", |result| result.display_label()),
            secondary_label: secondary.map(|result| result.display_label()),
            primary: primary_series(primary, |series| &series.server_context_switches_s),
            secondary: secondary_series(secondary, |series| &series.server_context_switches_s),
        },
    ));

    svg.push_str("</svg>");
    svg
}

fn render_series_panel(rect: PanelRect, series: SeriesPanel<'_>) -> String {
    let PanelRect { x, y, w, h } = rect;
    let SeriesPanel {
        title,
        unit,
        primary_label,
        secondary_label,
        primary,
        secondary,
    } = series;
    let mut out = String::new();
    out.push_str(&panel_card(x, y, w, h, PRIMARY_COLOR));
    out.push_str(&mono_text(
        x + 22.0,
        y + 32.0,
        18,
        700,
        PRIMARY_COLOR,
        title,
    ));

    let plot_x = x + 20.0;
    let plot_y = y + 56.0;
    let plot_w = w - 40.0;
    let plot_h = h - 108.0;
    let max_y = series_max(primary).max(series_max(secondary)).max(1.0);

    for idx in 0..=3 {
        let frac = idx as f64 / 3.0;
        let gy = plot_y + plot_h - frac * plot_h;
        out.push_str(&format!(
            r##"<line x1="{plot_x:.1}" y1="{gy:.1}" x2="{:.1}" y2="{gy:.1}" stroke="{BORDER_LIGHT}" stroke-width="1"/>"##,
            plot_x + plot_w
        ));
        out.push_str(&ui_text(
            plot_x + plot_w - 2.0,
            gy - 4.0,
            10,
            500,
            TEXT_MUTED,
            &format!("{:.1} {}", max_y * frac, unit),
        ));
    }

    out.push_str(&polyline(
        plot_x,
        plot_y,
        plot_w,
        plot_h,
        max_y,
        primary,
        PRIMARY_COLOR,
    ));
    out.push_str(&polyline(
        plot_x,
        plot_y,
        plot_w,
        plot_h,
        max_y,
        secondary,
        SECONDARY_COLOR,
    ));

    out.push_str(&mono_text(
        plot_x,
        y + h - 34.0,
        12,
        600,
        PRIMARY_COLOR,
        &format!(
            "{} avg {:.1} {} peak {:.1}",
            primary_label,
            series_avg(primary),
            unit,
            series_max(primary)
        ),
    ));
    if let Some(secondary_label) = secondary_label {
        out.push_str(&mono_text(
            plot_x,
            y + h - 18.0,
            12,
            600,
            SECONDARY_COLOR,
            &format!(
                "{} avg {:.1} {} peak {:.1}",
                secondary_label,
                series_avg(secondary),
                unit,
                series_max(secondary)
            ),
        ));
    }
    out
}

fn prepare_local_flamegraphs(
    results_dir: &Path,
    report: &RunReport,
) -> io::Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();

    for case in &report.cases {
        for result in &case.results {
            let key = result_key(case, result);
            let Some(perf_artifacts) = result
                .perf
                .as_ref()
                .and_then(|perf| perf.artifacts.as_ref())
            else {
                continue;
            };

            if let Some(path) = perf_artifacts.flamegraph_svg.as_deref() {
                let resolved = resolve_artifact_path(results_dir, path);
                if resolved.exists() {
                    out.insert(key.clone(), relative_display(results_dir, &resolved));
                    continue;
                }
            }

            let Some(script_path) = perf_artifacts.script_path.as_deref() else {
                continue;
            };
            let script_path = resolve_artifact_path(results_dir, script_path);
            if !script_path.exists() {
                continue;
            }

            let folded_path = perf_artifacts
                .folded_path
                .as_deref()
                .map(|path| resolve_artifact_path(results_dir, path))
                .unwrap_or_else(|| script_path.with_extension("folded"));
            let svg_path = perf_artifacts
                .flamegraph_svg
                .as_deref()
                .map(|path| resolve_artifact_path(results_dir, path))
                .unwrap_or_else(|| script_path.with_extension("svg"));

            if !svg_path.exists() && local_inferno_available() {
                if let Some(parent) = folded_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                if let Some(parent) = svg_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                generate_local_flamegraph(&script_path, &folded_path, &svg_path)?;
            }

            if svg_path.exists() {
                out.insert(key, relative_display(results_dir, &svg_path));
            }
        }
    }

    Ok(out)
}

fn sorted_results(case: &CaseReport) -> Vec<&ImplementationResult> {
    let mut out = case.results.iter().collect::<Vec<_>>();
    out.sort_by(|left, right| {
        compare_side_rank(left.compare_side)
            .cmp(&compare_side_rank(right.compare_side))
            .then_with(|| left.display_label().cmp(right.display_label()))
    });
    out
}

fn compare_side_rank(side: Option<CompareSide>) -> u8 {
    match side {
        Some(CompareSide::Left) => 0,
        Some(CompareSide::Right) => 1,
        None => 2,
    }
}

fn result_key(case: &CaseReport, result: &ImplementationResult) -> String {
    format!(
        "{}__{}",
        sanitize_label(&case.id),
        sanitize_label(result.display_label())
    )
}

fn case_has_os_series(case: &CaseReport) -> bool {
    case.results.iter().any(|result| {
        result
            .os
            .as_ref()
            .and_then(|os| os.series.as_ref())
            .is_some_and(|series| {
                !series.server_cpu_busy_pct.is_empty()
                    || !series.server_net_tx_mb_s.is_empty()
                    || !series.server_run_queue.is_empty()
                    || !series.server_context_switches_s.is_empty()
            })
    })
}

fn primary_series<'a>(
    result: Option<&'a ImplementationResult>,
    select: impl Fn(&'a OsSeries) -> &'a [f64],
) -> &'a [f64] {
    result
        .and_then(|result| result.os.as_ref())
        .and_then(|os| os.series.as_ref())
        .map(select)
        .unwrap_or(&[])
}

fn secondary_series<'a>(
    result: Option<&'a ImplementationResult>,
    select: impl Fn(&'a OsSeries) -> &'a [f64],
) -> &'a [f64] {
    result
        .and_then(|result| result.os.as_ref())
        .and_then(|os| os.series.as_ref())
        .map(select)
        .unwrap_or(&[])
}

fn accent(index: usize) -> (&'static str, &'static str) {
    match index % 4 {
        0 => (PRIMARY_COLOR, PRIMARY_FILL),
        1 => (SECONDARY_COLOR, SECONDARY_FILL),
        2 => (TERTIARY_COLOR, TERTIARY_FILL),
        _ => (QUATERNARY_COLOR, QUATERNARY_FILL),
    }
}

fn panel_card(x: f64, y: f64, w: f64, h: f64, accent: &str) -> String {
    format!(
        r##"<g><rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="10" fill="{SURFACE}" stroke="{BORDER_LIGHT}" stroke-width="1"/><rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="4" rx="10" fill="{accent}"/></g>"##
    )
}

fn metric_chip(x: f64, y: f64, label: &str, value: &str, accent: &str) -> String {
    let fill = if accent == SECONDARY_COLOR {
        SECONDARY_FILL
    } else if accent == TERTIARY_COLOR {
        TERTIARY_FILL
    } else if accent == QUATERNARY_COLOR {
        QUATERNARY_FILL
    } else {
        PRIMARY_FILL
    };
    format!(
        r##"<g><rect x="{x:.1}" y="{y:.1}" width="128" height="38" rx="4" fill="{fill}" stroke="{accent}" stroke-width="1"/><text x="{:.1}" y="{:.1}" font-family="monospace" font-size="12" font-weight="700" fill="{accent}">{}</text><text x="{:.1}" y="{:.1}" font-family="system-ui, -apple-system, sans-serif" font-size="10" font-weight="500" fill="{TEXT_PRIMARY}">{}</text></g>"##,
        x + 12.0,
        y + 14.0,
        xml_escape(label),
        x + 12.0,
        y + 29.0,
        xml_escape(value)
    )
}

fn metric_bar(x: f64, y: f64, w: f64, h: f64, stroke: &str, fill: &str) -> String {
    format!(
        r##"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="4" fill="{fill}" stroke="{stroke}" stroke-width="1.5"/>"##
    )
}

fn legend_chip(x: f64, y: f64, color: &str, fill: &str, label: &str) -> String {
    format!(
        r##"<g><rect x="{x:.1}" y="{y:.1}" width="120" height="28" rx="4" fill="{fill}" stroke="{BORDER_LIGHT}" stroke-width="1"/><rect x="{:.1}" y="{:.1}" width="12" height="12" rx="4" fill="{fill}" stroke="{color}" stroke-width="1.5"/><text x="{:.1}" y="{:.1}" font-family="monospace" font-size="12" font-weight="700" fill="{color}">{}</text></g>"##,
        x + 12.0,
        y + 8.0,
        x + 30.0,
        y + 18.0,
        xml_escape(label)
    )
}

fn polyline(x: f64, y: f64, w: f64, h: f64, max_y: f64, series: &[f64], color: &str) -> String {
    if series.is_empty() {
        return String::new();
    }

    if series.len() == 1 {
        let cy = y + h - (series[0] / max_y) * h;
        return format!(
            r##"<circle cx="{:.1}" cy="{cy:.1}" r="4" fill="{color}"/>"##,
            x + w / 2.0
        );
    }

    let mut points = String::new();
    for (idx, value) in series.iter().enumerate() {
        let frac = idx as f64 / (series.len() - 1) as f64;
        let px = x + frac * w;
        let py = y + h - (value / max_y) * h;
        if !points.is_empty() {
            points.push(' ');
        }
        write!(&mut points, "{px:.1},{py:.1}").unwrap();
    }

    format!(
        r##"<polyline fill="none" stroke="{color}" stroke-width="2.5" points="{points}" stroke-linejoin="round" stroke-linecap="round"/>"##
    )
}

fn ui_text(x: f64, y: f64, size: u32, weight: u32, fill: &str, value: &str) -> String {
    format!(
        r#"<text x="{x:.1}" y="{y:.1}" font-family="system-ui, -apple-system, sans-serif" font-size="{size}" font-weight="{weight}" fill="{fill}">{}</text>"#,
        xml_escape(value)
    )
}

fn mono_text(x: f64, y: f64, size: u32, weight: u32, fill: &str, value: &str) -> String {
    format!(
        r#"<text x="{x:.1}" y="{y:.1}" font-family="monospace" font-size="{size}" font-weight="{weight}" fill="{fill}">{}</text>"#,
        xml_escape(value)
    )
}

fn markdown_link(path: &str, label: &str) -> String {
    format!("[{label}](./{path})")
}

fn markdown_link_opt(path: Option<&str>, label: &str) -> String {
    path.map(|path| markdown_link(path, label))
        .unwrap_or_else(|| "-".into())
}

fn case_svg_filename(case_id: &str) -> String {
    format!("case-{}.svg", sanitize_label(case_id))
}

fn telemetry_svg_filename(case_id: &str) -> String {
    format!("case-{}-telemetry.svg", sanitize_label(case_id))
}

fn format_status_codes(metrics: &BenchmarkMetrics) -> String {
    if metrics.status_codes.is_empty() {
        return "-".into();
    }

    metrics
        .status_codes
        .iter()
        .map(|(status, count)| format!("{status}:{count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".into())
}

fn top_perf_label(result: &ImplementationResult) -> Option<String> {
    let hotspot = result
        .perf
        .as_ref()
        .and_then(|perf| perf.summary.as_ref())
        .and_then(|summary| summary.hotspots.first())?;
    Some(format!(
        "{:.2}% {} ({})",
        hotspot.pct,
        trim_text(&hotspot.symbol, 42),
        hotspot.shared_object
    ))
}

fn pct_delta(left: f64, right: f64) -> f64 {
    if right == 0.0 {
        0.0
    } else {
        ((left - right) / right) * 100.0
    }
}

fn series_avg(series: &[f64]) -> f64 {
    if series.is_empty() {
        0.0
    } else {
        series.iter().sum::<f64>() / series.len() as f64
    }
}

fn series_max(series: &[f64]) -> f64 {
    series.iter().copied().fold(0.0, f64::max)
}

fn trim_text(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        input.to_string()
    } else {
        let trimmed: String = input.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{trimmed}...")
    }
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

fn resolve_artifact_path(results_dir: &Path, artifact: &str) -> PathBuf {
    let path = PathBuf::from(artifact);
    if path.is_absolute() {
        path
    } else {
        results_dir.join(path)
    }
}

fn relative_display(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn local_inferno_available() -> bool {
    command_exists("inferno-collapse-perf") && command_exists("inferno-flamegraph")
}

fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn generate_local_flamegraph(
    script_path: &Path,
    folded_path: &Path,
    svg_path: &Path,
) -> io::Result<()> {
    let script_file = fs::File::open(script_path)?;
    let folded_file = fs::File::create(folded_path)?;
    let status = Command::new("inferno-collapse-perf")
        .stdin(Stdio::from(script_file))
        .stdout(Stdio::from(folded_file))
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "inferno-collapse-perf failed for {}",
            script_path.display()
        )));
    }

    let folded_file = fs::File::open(folded_path)?;
    let svg_file = fs::File::create(svg_path)?;
    let status = Command::new("inferno-flamegraph")
        .stdin(Stdio::from(folded_file))
        .stdout(Stdio::from(svg_file))
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(io::Error::other(format!(
            "inferno-flamegraph failed for {}",
            folded_path.display()
        )));
    }

    Ok(())
}

#[allow(dead_code)]
fn format_cpu(cpu: Option<&CpuSummary>) -> String {
    match cpu {
        Some(cpu) => format!(
            "{:.1}% / {:.1}% / {:.1}% / {:.1}%",
            cpu.user, cpu.system, cpu.iowait, cpu.idle
        ),
        None => "-".into(),
    }
}

#[allow(dead_code)]
fn format_net(net: Option<&NetSummary>) -> String {
    match net {
        Some(net) => format!(
            "{:.1} / {:.1} MB/s · retrans {:.2}/s",
            net.rx_kb_s / 1024.0,
            net.tx_kb_s / 1024.0,
            net.retrans_s
        ),
        None => "-".into(),
    }
}
