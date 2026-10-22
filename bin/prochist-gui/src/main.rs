mod actions;
mod app;
mod help;
mod source;
mod state;
mod table;
mod ui_tests;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use gpui_kit::*;
use prochist_core::{MockProvider, Pid, ProcessProvider, build_tree};

use crate::actions::*;
use crate::app::{Init, Phg};
use crate::source::Source;

#[derive(Parser)]
#[command(
    name = "phg",
    version,
    about = "Interactive GUI for the process tree around a PID"
)]
struct Cli {
    /// Show the process tree around PID.
    #[arg(short, long, conflicts_with = "file")]
    pid: Option<Pid>,

    /// Show the processes that have FILE or DIR open.
    #[arg(short, long, value_name = "FILE|DIR")]
    file: Option<PathBuf>,

    /// Load a JSON process snapshot instead of querying the OS (testing).
    #[arg(long, value_name = "FILE", hide = true)]
    snapshot: Option<PathBuf>,
}

fn fail(message: &str) -> ExitCode {
    eprintln!("phg: error: {message}");
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let source = match &cli.snapshot {
        Some(path) => match MockProvider::from_json_file(path) {
            Ok(provider) => match provider.snapshot() {
                Ok(processes) => Source::Fixture(processes),
                Err(e) => return fail(&format!("cannot read snapshot {}: {e}", path.display())),
            },
            Err(e) => return fail(&format!("cannot load snapshot {}: {e}", path.display())),
        },
        None => Source::Live,
    };

    let snapshot = match source.snapshot() {
        Ok(snapshot) => snapshot,
        Err(e) => return fail(&format!("cannot list processes: {e}")),
    };

    let (title, initial) = if let Some(path) = cli.file {
        let pids = match source.holders(&path) {
            Ok(pids) => pids,
            Err(e) => return fail(&format!("cannot query holders of {}: {e}", path.display())),
        };
        let holders = state::resolve_holders(&snapshot, &pids);
        (
            format!("phg — holders of {}", path.display()),
            Init::Holders {
                snapshot,
                path,
                holders,
            },
        )
    } else if let Some(pid) = cli.pid {
        if let Err(e) = build_tree(&snapshot, pid) {
            return fail(&format!("{e}"));
        }
        (format!("phg — process {pid}"), Init::Tree { snapshot, pid })
    } else {
        ("phg — processes".to_string(), Init::Processes { snapshot })
    };

    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        bind_keys(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1100.), px(750.)), cx)),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        open_window(options, cx, move |window, cx| {
            cx.new(|cx| Phg::new(window, cx, source, initial))
        })
        .expect("failed to open window");
    });

    ExitCode::SUCCESS
}

fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        // Main pane (tree / process tables).
        KeyBinding::new("j", MoveDown, Some("PhgMain")),
        KeyBinding::new("k", MoveUp, Some("PhgMain")),
        KeyBinding::new("ctrl-d", HalfPageDown, Some("PhgMain")),
        KeyBinding::new("ctrl-u", HalfPageUp, Some("PhgMain")),
        KeyBinding::new("ctrl-f", FullPageDown, Some("PhgMain")),
        KeyBinding::new("ctrl-b", FullPageUp, Some("PhgMain")),
        KeyBinding::new("g g", MoveStart, Some("PhgMain")),
        KeyBinding::new("shift-g", MoveEnd, Some("PhgMain")),
        KeyBinding::new("enter", FocusSelected, Some("PhgMain")),
        KeyBinding::new("f", FocusSelected, Some("PhgMain")),
        KeyBinding::new("escape", RestoreFocus, Some("PhgMain")),
        KeyBinding::new("backspace", RestoreFocus, Some("PhgMain")),
        KeyBinding::new("/", FocusSearch, Some("PhgMain")),
        KeyBinding::new("n", NextMatch, Some("PhgMain")),
        KeyBinding::new("shift-n", PrevMatch, Some("PhgMain")),
        KeyBinding::new("r", Refresh, Some("PhgMain")),
        KeyBinding::new("shift-/", ToggleHelp, Some("PhgMain")),
        KeyBinding::new("q", QuitApp, Some("PhgMain")),
        KeyBinding::new("tab", FocusDetails, Some("PhgMain")),
        KeyBinding::new("i", FocusDetails, Some("PhgMain")),
        KeyBinding::new("y", YankNamePid, Some("PhgMain")),
        KeyBinding::new("shift-y", YankCommand, Some("PhgMain")),
        // Details pane.
        KeyBinding::new("j", DetailDown, Some("PhgDetails")),
        KeyBinding::new("k", DetailUp, Some("PhgDetails")),
        KeyBinding::new("y", YankField, Some("PhgDetails")),
        KeyBinding::new("shift-y", YankNamePid, Some("PhgDetails")),
        KeyBinding::new("tab", FocusMain, Some("PhgDetails")),
        KeyBinding::new("escape", FocusMain, Some("PhgDetails")),
        KeyBinding::new("enter", FocusMain, Some("PhgDetails")),
        KeyBinding::new("q", QuitApp, Some("PhgDetails")),
        KeyBinding::new("r", Refresh, Some("PhgDetails")),
        KeyBinding::new("shift-/", ToggleHelp, Some("PhgDetails")),
        // Search box.
        KeyBinding::new("escape", CancelSearch, Some("PhgSearch")),
    ]);
}
