# prochist-tui (`phi`) — AGENTS.md

Interactive TUI over `ProcessTree`. ratatui 0.30 + crossterm 0.29. Like `ph`,
it mirrors the clap provider wiring (including the hidden `--snapshot <FILE>`
testing flag) and the `phi: error: ...` / exit-code-1 error convention.

## Commands

- `cargo run --bin phi -- [-p PID]` or `cargo run --bin phi -- -f FILE|DIR` — run against the live OS
- `cargo run --bin phi -- --snapshot tests/...json [-f PATH]` — run against a fixture
- `cargo test -p prochist-tui` — unit tests (no trycmd here; `ph` owns e2e)

## Structure

- `src/main.rs` — clap, provider selection, terminal lifecycle
  (`TerminalGuard` + panic hook restoring the terminal), event loop. Thin:
  translates keys into `App::handle_key` and performs `Effect`s.
- `src/app.rs` — pure application state: two views (`View::Tree` /
  `View::Holders`), row list, selection/scroll, pane focus (`Tree`/`Detail`),
  focus stack (`FocusEntry::Process` / `FocusEntry::Holders`; Enter/`f`
  re-roots the tree around the selected PID, `Esc`/`Backspace` pops back),
  action popup (`a`), help overlay, vim keymap (`j/k`, `Ctrl-d/u`, `Ctrl-f/b`,
  `gg/G`, `y`/`Y` yank, `?` help, `q` quit). `handle_key` returns
  `Option<Effect>` instead of doing I/O — keep it side-effect free and tested.
  Re-rooting is requested via `Effect::Focus`/`Effect::Restore`; `main.rs`
  rebuilds the tree and calls `apply_focus`/`enter_tree_from_holders`/
  `apply_restore` only on success (a failed `Restore` drops the popped stack
  entry — acceptable, the user can re-focus). Popping a `FocusEntry::Holders`
  flips back to the cached `HoldersState` internally — no provider roundtrip.
- `src/ui.rs` — ratatui rendering: tree pane or holders pane (Fill), details
  pane (fixed 7), status bar (mode tag, position, focus breadcrumb), action
  popup, help overlay. Tested with `TestBackend`.
- `src/clipboard.rs` — OSC 52 yank (`ESC ] 52 ; c ; <base64> ST`), with an
  in-crate base64 encoder. No clipboard crate by design (no platform deps,
  works over SSH); terminals without OSC 52 support silently ignore it.

## Conventions / invariants

- New keybinding = branch in `app.rs` + row in `HELP_LINES` + unit test +
  `docs/tui.md` table. All four, or the change is incomplete.
- `app.rs` must stay backend-agnostic: no `execute!`/stdout writes there.
- `r` refresh is wired in `main.rs` (needs the provider) and is view-aware:
  tree view re-snapshots around `app.focused_pid()` (the focused process, not
  the startup PID; `App::replace_tree` preserves the selected PID), holders
  view re-runs `provider.holders(path)` + `App::replace_holders` (preserves
  selection by PID, empty-safe).
- Selection/details access goes through `App::selected_info()` / `position()`
  — view-aware; never index `app.rows[app.selected]` directly in `ui.rs`.
- Detail pane height is fixed at 7 (5 fields + borders); adding a field to
  `FIELD_LABELS` means bumping that constraint in `ui.rs`.
