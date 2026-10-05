# prochist-core — AGENTS.md

Data model, `ProcessProvider` trait, and tree building. No user-facing output lives here by design (a future GUI reuses this model) — do not add printing/formatting code.

## Commands

- `cargo test -p prochist-core` — unit tests only (all in-module `#[cfg(test)]`)
- `cargo clippy -p prochist-core`

## Module map

- `model.rs` — `ProcessInfo { pid, ppid, name, command?, exe?, open_files }` (serde; `command`/`exe`/`open_files` use `#[serde(default)]` so old snapshot JSON without them still parses; only `MockProvider` populates `open_files`), `ProcessTree { ancestors, current, children }`. `Pid = u32`.
- `provider.rs` — trait with required `snapshot() -> io::Result<Vec<ProcessInfo>>` and `holders(path) -> io::Result<Vec<Pid>>` (file-holder query, default impl returns `Unsupported`; Linux/Windows/Mock override it).
- `tree.rs` — `build_tree(snapshot, pid)`. See invariants below.
- `linux.rs` — dev/verification backend, parses `/proc/<pid>/stat` + `cmdline` + `exe` symlink. `holders` scans `/proc/*/fd/*` (dev+inode match for files, canonical prefix match for dirs). Reference implementation for new backends.
- `windows.rs` — shipping target, Toolhelp snapshot backend (`windows-sys`). `holders` uses Restart Manager (files only). PID-reuse caveats in `docs/windows-notes.md` (read before changing).
- `mock.rs` — `MockProvider` from a JSON array of `ProcessInfo`; used by CLI e2e tests via `ph --snapshot`. `holders` is answered from each process's `open_files` (exact match, or `<query>/` prefix for directory-style queries).

## `build_tree` invariants (tested in `tree.rs`)

- Ancestors returned root-first; walk stops when parent is absent from the snapshot (dead parent) — not an error.
- Cycle guard via a `seen` set: do NOT add redundant cycle/missing-parent checks in providers.
- Children are direct only, sorted by PID, and exclude the target itself (self-parented processes exist).
- Unknown target PID is the only error: `TreeError::NotFound(pid)`.

## Adding a platform backend

1. Implement `ProcessProvider` in a new module; gate it with `#[cfg(target_os = ...)]` in `lib.rs` and wire `default_provider()`.
2. Populate `name`; `command`/`exe` are `Option` — leave `None` when unavailable rather than failing the whole snapshot (protected processes, mid-scan exits).
3. Skip entries that vanish mid-scan; never let one bad entry fail the snapshot (see `linux.rs` for the pattern).
4. Override `holders` only if the platform has a sane file-holder query; the default `Unsupported` impl is acceptable.
5. `lib.rs` has a `compile_error!` for non-Linux/non-Windows — update it when adding a target.
