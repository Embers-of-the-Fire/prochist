use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use prochist_core::{MockProvider, ProcessProvider, TreeError, build_tree, default_provider};

mod render;

#[derive(Parser)]
#[command(name = "ph", version, about = "Print the process tree around a PID")]
struct Cli {
    /// Process ID to inspect [default: current process]
    pid: Option<u32>,

    /// Load a mocked process snapshot from a JSON file (testing only)
    #[arg(long, hide = true, value_name = "FILE")]
    snapshot: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

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

    let pid = cli.pid.unwrap_or_else(std::process::id);
    match build_tree(&processes, pid) {
        Ok(tree) => {
            print!("{}", render::render(&tree));
            ExitCode::SUCCESS
        }
        Err(TreeError::NotFound(pid)) => {
            eprintln!("ph: error: no such process: {pid}");
            ExitCode::FAILURE
        }
    }
}
