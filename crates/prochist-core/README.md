# prochist-core

Process snapshot model, provider trait, and tree building for prochist.

This crate is deliberately free of user-facing output: it exposes only the data
model and tree logic, so a future GUI frontend can reuse the same model and
render it differently.

## Contents

- `model` — `ProcessInfo { pid, ppid, name, command?, exe? }` (serde),
  `ProcessTree { ancestors, current, children }`. `Pid = u32`.
- `provider` — the single-method trait
  `ProcessProvider::snapshot() -> io::Result<Vec<ProcessInfo>>`, implemented by
  all platform backends and test mocks.
- `tree` — `build_tree(snapshot, pid)`: walks the `ppid` chain upward with a
  cycle guard and collects direct children sorted by PID. Ancestors are
  returned root-first; a missing parent is not an error; an unknown target PID
  is the only error (`TreeError::NotFound`).
- `linux` — development/verification backend, parses `/proc/<pid>/stat`.
  Reference implementation for new backends.
- `windows` — the shipping target, currently stubbed (`todo!()`). Plan and
  PID-reuse caveats: [`docs/windows-notes.md`](../../docs/windows-notes.md).
- `mock` — `MockProvider`, loads a snapshot from JSON for tests.

Supported targets are Linux and Windows; other targets fail to compile with a
`compile_error!` in `lib.rs`.

## Testing

```sh
cargo test -p prochist-core
```

All tests are in-module unit tests (`#[cfg(test)]`).
