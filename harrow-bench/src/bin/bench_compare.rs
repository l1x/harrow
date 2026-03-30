use std::path::PathBuf;

use harrow_bench::harness::runner::{CommonRunConfig, CompareRunConfig, run_compare};
use harrow_bench::harness::spec::DeploymentMode;

fn usage() -> ! {
    eprintln!(
        "Usage: bench-compare --left ID --right ID --suite PATH [OPTIONS]\n\
         \n\
         Required:\n\
         \x20 --left ID                 Left implementation id\n\
         \x20 --right ID                Right implementation id\n\
         \x20 --suite PATH              Suite manifest path\n\
         \n\
         Optional:\n\
         \x20 --case ID                 Run only one case from the suite (repeatable)\n\
         \x20 --mode MODE               local|remote (default: local)\n\
         \x20 --server-ssh HOST         Server SSH host (remote mode)\n\
         \x20 --client-ssh HOST         Client SSH host (remote mode)\n\
         \x20 --server-private-ip IP    Explicit target IP for rendered templates\n\
         \x20 --ssh-user USER           SSH user (default: alpine)\n\
         \x20 --port PORT               Target port (default: 3090)\n\
         \x20 --duration SECS           Default duration in seconds (default: 30)\n\
         \x20 --warmup SECS             Default warmup in seconds (default: 5)\n\
         \x20 --registry PATH           Implementation registry (default: harrow-bench/implementations.toml)\n\
         \x20 --results-dir DIR         Override results directory\n\
         \x20 --no-build-missing        Fail instead of building missing local images\n"
    );
    std::process::exit(1);
}

fn main() {
    let mut common = CommonRunConfig::default();
    let mut left_id: Option<String> = None;
    let mut right_id: Option<String> = None;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--left" => {
                left_id = Some(args.get(i + 1).unwrap_or_else(|| usage()).clone());
                i += 2;
            }
            "--right" => {
                right_id = Some(args.get(i + 1).unwrap_or_else(|| usage()).clone());
                i += 2;
            }
            "--suite" => {
                common.suite_path = PathBuf::from(args.get(i + 1).unwrap_or_else(|| usage()));
                i += 2;
            }
            "--case" => {
                common
                    .case_filters
                    .push(args.get(i + 1).unwrap_or_else(|| usage()).clone());
                i += 2;
            }
            "--mode" => {
                common.deployment_mode =
                    DeploymentMode::parse(args.get(i + 1).unwrap_or_else(|| usage()))
                        .unwrap_or_else(|| usage());
                i += 2;
            }
            "--server-ssh" => {
                common.server_ssh = Some(args.get(i + 1).unwrap_or_else(|| usage()).clone());
                i += 2;
            }
            "--client-ssh" => {
                common.client_ssh = Some(args.get(i + 1).unwrap_or_else(|| usage()).clone());
                i += 2;
            }
            "--server-private-ip" => {
                common.server_private_ip = Some(args.get(i + 1).unwrap_or_else(|| usage()).clone());
                i += 2;
            }
            "--ssh-user" => {
                common.ssh_user = args.get(i + 1).unwrap_or_else(|| usage()).clone();
                i += 2;
            }
            "--port" => {
                common.port = args
                    .get(i + 1)
                    .unwrap_or_else(|| usage())
                    .parse()
                    .unwrap_or_else(|_| usage());
                i += 2;
            }
            "--duration" => {
                common.duration_secs = args
                    .get(i + 1)
                    .unwrap_or_else(|| usage())
                    .parse()
                    .unwrap_or_else(|_| usage());
                i += 2;
            }
            "--warmup" => {
                common.warmup_secs = args
                    .get(i + 1)
                    .unwrap_or_else(|| usage())
                    .parse()
                    .unwrap_or_else(|_| usage());
                i += 2;
            }
            "--registry" => {
                common.registry_path = PathBuf::from(args.get(i + 1).unwrap_or_else(|| usage()));
                i += 2;
            }
            "--results-dir" => {
                common.results_dir =
                    Some(PathBuf::from(args.get(i + 1).unwrap_or_else(|| usage())));
                i += 2;
            }
            "--no-build-missing" => {
                common.build_missing = false;
                i += 1;
            }
            "-h" | "--help" => usage(),
            _ => usage(),
        }
    }

    let left_id = left_id.unwrap_or_else(|| usage());
    let right_id = right_id.unwrap_or_else(|| usage());
    if !common.suite_path.exists() {
        eprintln!("suite file not found: {}", common.suite_path.display());
        std::process::exit(1);
    }

    match run_compare(CompareRunConfig {
        common,
        left_id,
        right_id,
    }) {
        Ok(_) => {}
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}
