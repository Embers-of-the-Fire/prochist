use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use prochist_core::{
    MockProvider, Pid, ProcessProvider, ProcessTree, TreeError, build_tree, default_provider,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

mod app;
mod clipboard;
mod ui;

use app::{App, Effect, Focus};

#[derive(Parser)]
#[command(
    name = "phi",
    version,
    about = "Interactive TUI for the process tree around a PID"
)]
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
                eprintln!("phi: error: cannot load snapshot {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        },
        None => default_provider(),
    };

    let processes = match provider.snapshot() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("phi: error: cannot enumerate processes: {e}");
            return ExitCode::FAILURE;
        }
    };

    let pid = cli.pid.unwrap_or_else(std::process::id);
    let tree = match build_tree(&processes, pid) {
        Ok(tree) => tree,
        Err(TreeError::NotFound(pid)) => {
            eprintln!("phi: error: no such process: {pid}");
            return ExitCode::FAILURE;
        }
    };

    match run(provider.as_ref(), tree) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("phi: error: {e}");
            ExitCode::FAILURE
        }
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        install_panic_hook();
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original(info);
    }));
}

fn run(provider: &dyn ProcessProvider, tree: ProcessTree) -> io::Result<()> {
    let _guard = TerminalGuard::enter()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;

    let mut app = App::new(&tree);
    while !app.should_quit {
        terminal.draw(|f| ui::draw(f, &mut app))?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if key.code == KeyCode::Char('r')
                && !app.show_help
                && !app.show_actions
                && app.focus == Focus::Tree
            {
                refresh(provider, &mut app);
                continue;
            }
            match app.handle_key(key) {
                Some(Effect::Copy(text)) => match clipboard::copy(&text) {
                    Ok(()) => app.note(format!("yanked: {}", truncate(&text, 40))),
                    Err(e) => app.note(format!("yank failed: {e}")),
                },
                Some(Effect::Focus(pid)) => focus_on(provider, pid, true, &mut app),
                Some(Effect::Restore(pid)) => focus_on(provider, pid, false, &mut app),
                None => {}
            }
        }
    }
    Ok(())
}

fn rebuild(provider: &dyn ProcessProvider, pid: Pid) -> Result<ProcessTree, String> {
    let snapshot = provider
        .snapshot()
        .map_err(|e| format!("cannot enumerate processes: {e}"))?;
    build_tree(&snapshot, pid).map_err(|TreeError::NotFound(pid)| format!("no such process: {pid}"))
}

fn focus_on(provider: &dyn ProcessProvider, pid: Pid, push: bool, app: &mut App) {
    match rebuild(provider, pid) {
        Ok(tree) => {
            if push {
                app.apply_focus(&tree);
            } else {
                app.apply_restore(&tree);
            }
            app.note(format!("focused: {pid}"));
        }
        Err(message) => app.note(message),
    }
}

fn refresh(provider: &dyn ProcessProvider, app: &mut App) {
    let pid = app.focused_pid();
    match rebuild(provider, pid) {
        Ok(tree) => {
            app.replace_tree(&tree);
            app.note("refreshed");
        }
        Err(message) => app.note(message),
    }
}

fn truncate(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let shortened: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        format!("{shortened}…")
    } else {
        shortened
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_shortens_long_text() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello…");
    }
}
