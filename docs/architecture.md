# Architecture

prochist is a Rust workspace with the following layout:

```
crates/prochist-core   # data model, provider trait, tree building, platform backends
bin/prochist-cli       # the `ph` binary (clap 4 derive) + tree rendering
bin/prochist-tui       # the `phi` binary — interactive ratatui/crossterm frontend
docs/                  # design notes
```

## prochist-core

- `model` — `ProcessInfo { pid, ppid, name }`, `ProcessTree { ancestors, current, children }`.
- `provider` — the `ProcessProvider` trait (`fn snapshot() -> io::Result<Vec<ProcessInfo>>`).
  All platform backends and test mocks implement this single trait.
- `tree` — `build_tree(snapshot, pid)`: walks the `ppid` chain upward with a cycle
  guard (protects against PID reuse / pathological snapshots) and collects direct
  children sorted by PID.
- `linux` — development/verification backend, parses `/proc/<pid>/stat`.
- `windows` — the shipping target; Toolhelp snapshot
  (`CreateToolhelp32Snapshot` + `Process32FirstW`/`Process32NextW`), with
  best-effort `exe` paths via `QueryFullProcessImageNameW`. See `windows-notes.md`.
- `mock` — `MockProvider`, loads a snapshot from JSON for tests.

## prochist-cli

The CLI crate owns all user-facing formatting (`render.rs`). Tree rendering is
deliberately kept out of `prochist-core` so a future GUI frontend can render the
same `ProcessTree` model differently.

Mocked input for end-to-end tests is injected via the hidden flag
`ph --snapshot <FILE.json>`, which swaps the OS provider for `MockProvider`.
The flag is hidden from `--help` and intended for testing only.

## prochist-tui

`phi` is the interactive frontend (ratatui + crossterm) over the same
`ProcessTree` model. Key handling lives in `app.rs` as pure state transitions
(returning `Effect`s such as `Copy`); side effects (terminal I/O, clipboard)
stay in `main.rs`/`clipboard.rs` so the app logic is unit-testable. Yanking
uses the OSC 52 escape sequence instead of a clipboard crate. See `tui.md`.
