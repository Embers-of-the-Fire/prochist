# prochist-gui (`phg`) — AGENTS.md

Desktop GUI over `prochist-core`, built with `gpui-kit` 0.8 (gpui-pre 0.3.9
snapshot). Same clap surface as `phi` (`-p` / `-f`, hidden `--snapshot`) and
the same `phg: error: ...` / exit-code-1 convention for application errors
(clap argument errors use clap's usage error / exit code 2).

## Commands

- `cargo run --bin phg -- [-p PID]` or `cargo run --bin phg -- -f FILE|DIR`
- `cargo run --bin phg -- --snapshot <fixtures/...json> [-p PID]` — fixture mode
- `cargo test -p prochist-gui` — state unit tests + headless UI tests
  (`src/ui_tests.rs`, via `gpui-kit/test-support` dev-dependency)

## Structure

- `src/main.rs` — clap, `Source` construction, keybinding table
  (`bind_keys`), window bootstrap. Keep it thin.
- `src/source.rs` — `Source::{Live, Fixture}`; Clone + Send so provider
  calls can move onto the background executor. `Live` re-creates
  `default_provider()` per call.
- `src/state.rs` — pure, gpui-free logic: `display_name`/`fields`/search
  parsing (ported from the TUI, keep semantics in sync), `build_subtree`
  (full descendant tree rooted at the outermost ancestor), `flatten`,
  `find_matches`, `resolve_holders`. Unit-tested.
- `src/app.rs` — the `Phg` root view: three views (`View::{Tree, Processes,
  Holders}`), focus stack, selection, details pane, status bar, help dialog,
  async refresh/focus via `cx.spawn_in`. Selection and details are read live
  from the component entities in `render` (Tree has no selection events);
  the root observes each entity to re-render.
- `src/table.rs` — `ProcessTableDelegate` (gpui-component `DataTable`)
  shared by the processes and holders views; owns filter + sort state and
  the right-click menu.

## Conventions / invariants

- Keybinding change = entry in `bind_keys` (main.rs) + `HELP_LINES` row
  (help.rs) + `docs/gui.md` table + a test where feasible. Contexts:
  `PhgMain` (list/tree), `PhgDetails`, `PhgSearch` — single-letter bindings
  must stay out of any context that contains the search input.
- Mouse is a first-class input: every keyboard action needs a pointer path
  (click / double-click / context menu / toolbar button).
- Component keyboard collisions are deliberate: in the tree, `Enter` toggles
  nodes and `Esc` belongs to the app; in tables, `Esc` clears the selection
  (DataTable owns it) and `Backspace` restores focus.
- Never block the UI thread on provider calls — `snapshot()`/`holders()`
  go through `cx.spawn_in` + `background_executor`, results are applied
  view-aware (preserve selection by PID, filter, tree expansion).
- The tree's expansion state is mirrored in `TreeView.expanded` (synced via
  `TreeEvent`) so refresh can rebuild `TreeItem`s without collapsing.
- gpui-kit version bumps: re-check `Cargo.lock` resolution — enabling
  features can pull the optional `gpui-fast-*` graph into the lockfile,
  which pins `cocoa =0.26.0` / `unicode-properties =0.1.3` (fixed via
  `cargo update -p cocoa@<newer> -p unicode-properties@<newer>`).
