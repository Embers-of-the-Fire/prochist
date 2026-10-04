# AGENTS.md

Rust workspace. `ph` — prints the process tree around a PID. Windows is the shipping target; Linux is dev/verification only.

## Commands

- `cargo test` — full suite (unit + trycmd end-to-end)
- `cargo test -p prochist-cli` — end-to-end only
- Bless trycmd fixtures after intentional output changes: `TRYCMD=overwrite cargo test`
- Run the binary: `cargo run --bin ph -- [flags] [PID]`

## Layout / boundaries

- `crates/prochist-core` — model, `ProcessProvider` trait, `build_tree`, platform backends. No formatting/rendering here by design (a future GUI reuses the same model). Details: `crates/prochist-core/AGENTS.md`.
- `bin/prochist-cli` — the `ph` binary (clap 4 derive); owns all user-facing output in `render.rs`. Details: `bin/prochist-cli/AGENTS.md`.
- `docs/architecture.md`, `docs/cli.md`, `docs/windows-notes.md` — keep in sync when behavior changes.

## Testing

- E2E tests are trycmd cases in `bin/prochist-cli/tests/fixtures/*.toml`, each pointing at a `*.snapshot.json` fed via the hidden `ph --snapshot <FILE>` flag (swaps the OS provider for `MockProvider`; never document it in `--help`).
- New CLI behavior = new `.toml` fixture + `.snapshot.json`; run `TRYCMD=overwrite cargo test` then review the diff.

## Platform notes

- `crates/prochist-core/src/lib.rs` has `compile_error!` for non-Linux/non-Windows targets.
- `windows.rs` is stubbed with `todo!()`; Linux backend (parses `/proc/<pid>/stat`) is the reference. Windows plan and PID-reuse caveats: `docs/windows-notes.md`.
- `build_tree` already guards against ancestor cycles and missing parents — do not add redundant checks in providers.

## Conventions

- Edition 2024, workspace-managed deps in root `Cargo.toml` — add deps there, use `dep.workspace = true` in members.
- Validation before done: `cargo fmt --check`, `cargo clippy`, `cargo test`.
