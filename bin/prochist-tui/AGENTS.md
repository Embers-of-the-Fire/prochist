# prochist-tui (`phi`) — AGENTS.md

Interactive TUI over `ProcessTree`. ratatui 0.30 + crossterm 0.29. Like `ph`,
it mirrors the clap provider wiring (including the hidden `--snapshot <FILE>`
testing flag) and the `phi: error: ...` / exit-code-1 error convention.

## Commands

- `cargo run --bin phi -- [PID]` — run against the live OS
- `cargo run --bin phi -- --snapshot tests/...json` — run against a fixture
- `cargo test -p prochist-tui` — unit tests (no trycmd here; `ph` owns e2e)

## Structure

- `src/main.rs` — clap, provider selection, terminal lifecycle
  (`TerminalGuard` + panic hook restoring the terminal), event loop. Thin:
  translates keys into `App::handle_key` and performs `Effect`s.
- `src/app.rs` — pure application state: row list, selection/scroll, focus
  (`Tree`/`Detail`), help overlay, vim keymap (`j/k`, `Ctrl-d/u`, `Ctrl-f/b`,
  `gg/G`, `y`/`Y` yank, `?` help, `q` quit). `handle_key` returns
  `Option<Effect>` instead of doing I/O — keep it side-effect free and tested.
- `src/ui.rs` — ratatui rendering: tree pane (Fill), details pane (fixed 7),
  status bar, help overlay. Tested with `TestBackend`.
- `src/clipboard.rs` — OSC 52 yank (`ESC ] 52 ; c ; <base64> ST`), with an
  in-crate base64 encoder. No clipboard crate by design (no platform deps,
  works over SSH); terminals without OSC 52 support silently ignore it.

## Conventions / invariants

- New keybinding = branch in `app.rs` + row in `HELP_LINES` + unit test +
  `docs/tui.md` table. All four, or the change is incomplete.
- `app.rs` must stay backend-agnostic: no `execute!`/stdout writes there.
- `r` refresh is wired in `main.rs` (needs the provider); `App::replace_tree`
  preserves the selected PID across refreshes.
- Detail pane height is fixed at 7 (5 fields + borders); adding a field to
  `FIELD_LABELS` means bumping that constraint in `ui.rs`.
