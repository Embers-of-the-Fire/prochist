# Architecture

prochist is a Rust workspace with the following layout:

```
crates/prochist-core   # data model, provider trait, tree building, platform backends
bin/prochist-cli       # the `ph` binary (clap 4 derive) + tree rendering
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
- `windows` — the shipping target; stubbed (`todo!()`) until the Toolhelp backend
  lands. See `windows-notes.md`.
- `mock` — `MockProvider`, loads a snapshot from JSON for tests.

## prochist-cli

The CLI crate owns all user-facing formatting (`render.rs`). Tree rendering is
deliberately kept out of `prochist-core` so a future GUI frontend can render the
same `ProcessTree` model differently.

Mocked input for end-to-end tests is injected via the hidden flag
`ph --snapshot <FILE.json>`, which swaps the OS provider for `MockProvider`.
The flag is hidden from `--help` and intended for testing only.
