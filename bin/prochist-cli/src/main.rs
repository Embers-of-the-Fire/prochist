use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use prochist_core::{
    MockProvider, ProcessInfo, ProcessProvider, TreeError, build_tree, default_provider,
};

mod render;

use render::RenderOptions;

#[derive(Parser)]
#[command(name = "ph", version, about = "Print the process tree around a PID")]
struct Cli {
    /// Process ID to inspect [default: current process]
    #[arg(short = 'p', long, value_name = "PID", conflicts_with = "file")]
    pid: Option<u32>,

    /// List processes that have FILE or DIR open
    #[arg(short = 'f', long, value_name = "PATH")]
    file: Option<PathBuf>,

    /// Show the tree with ASCII connectors instead of Unicode
    #[arg(short = 'A', long)]
    ascii: bool,

    /// Show the full command line under each process
    #[arg(short = 'L', long)]
    long: bool,

    /// Show the full executable path instead of the binary name
    #[arg(short = 'E', long)]
    executable: bool,

    /// Show at most N ancestors, omitting the oldest
    #[arg(short = 'M', long, value_name = "N", conflicts_with = "file")]
    max_ancestors: Option<usize>,

    /// Show at most N children (or file holders), omitting the rest
    #[arg(short = 'C', long, value_name = "N")]
    max_children: Option<usize>,

    /// Search processes (only available in the phi TUI)
    #[arg(short = 's', long, value_name = "QUERY")]
    search: Option<String>,

    /// Load a mocked process snapshot from a JSON file (testing only)
    #[arg(long, hide = true, value_name = "FILE")]
    snapshot: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.search.is_some() {
        eprintln!(
            "ph: error: --search is only available in the TUI (phi); use 'ps -aux | grep <pattern>' to search from the CLI"
        );
        return ExitCode::FAILURE;
    }

    let provider: Box<dyn ProcessProvider> = match &cli.snapshot {
        Some(path) => match MockProvider::from_json_file(path) {
            Ok(p) => Box::new(p),
            Err(e) => {
                eprintln!("ph: error: cannot load snapshot {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        },
        None => default_provider(),
    };

    let processes = match provider.snapshot() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("ph: error: cannot enumerate processes: {e}");
            return ExitCode::FAILURE;
        }
    };

    let opts = RenderOptions {
        ascii: cli.ascii,
        long: cli.long,
        executable: cli.executable,
        max_ancestors: cli.max_ancestors,
        max_children: cli.max_children,
    };

    if let Some(path) = &cli.file {
        return file_mode(provider.as_ref(), &processes, path, &opts);
    }

    let pid = cli.pid.unwrap_or_else(std::process::id);
    match build_tree(&processes, pid) {
        Ok(tree) => {
            print!("{}", render::render(&tree, &opts));
            ExitCode::SUCCESS
        }
        Err(TreeError::NotFound(pid)) => {
            eprintln!("ph: error: no such process: {pid}");
            ExitCode::FAILURE
        }
    }
}

fn file_mode(
    provider: &dyn ProcessProvider,
    processes: &[ProcessInfo],
    path: &Path,
    opts: &RenderOptions,
) -> ExitCode {
    let pids = match provider.holders(path) {
        Ok(pids) => pids,
        Err(e) => {
            eprintln!("ph: error: {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };
    if pids.is_empty() {
        eprintln!("ph: error: no process has {} open", path.display());
        return ExitCode::FAILURE;
    }
    let mut holders: Vec<ProcessInfo> = pids
        .iter()
        .filter_map(|pid| processes.iter().find(|p| p.pid == *pid))
        .cloned()
        .collect();
    holders.sort_by_key(|p| p.pid);
    print!("{}", render::render_holders(path, &holders, opts));
    ExitCode::SUCCESS
}
