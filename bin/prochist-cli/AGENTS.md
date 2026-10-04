# prochist-cli (`ph`) — AGENTS.md

The `ph` binary. Owns ALL user-facing output (`render.rs`); `prochist-core` must stay output-free.

## Commands

- `cargo run --bin ph -- [flags] [PID]` — run against the live OS (Linux backend)
- `cargo test -p prochist-cli` — trycmd end-to-end suite
- `TRYCMD=overwrite cargo test -p prochist-cli` — bless fixture outputs after an intentional change, then `git diff` the fixtures before committing

## Structure

- `src/main.rs` — clap 4 derive CLI, provider selection, error → exit-code mapping. Keep it thin; formatting decisions go in `render.rs`.
- `src/render.rs` — pure `render(&ProcessTree, &RenderOptions) -> String`. Unit-tested with exact-string assertions.
- `tests/cli.rs` — single trycmd runner over `tests/fixtures/*.toml`.
- `tests/fixtures/*.toml` + `*.snapshot.json` — one case per behavior.

## Conventions / invariants

- Exit codes: `0` tree printed; `1` unknown PID, unreadable snapshot, or provider failure. Errors go to stderr as `ph: error: ...`, never panic.
- `--snapshot <FILE>` is `hide = true` — testing only, never document it in `--help` output (it is documented in `docs/cli.md` for devs).
- Default PID = `std::process::id()` (ph inspects itself); fixture `default-self.toml` covers this.
- Name shown = basename of `exe` when available, else `name` from the provider; `-E/--executable` shows the full `exe` path. Binary name is never truncated.
- Omitted entries print `... N process(es) omitted` (singular/plural handled in `render.rs`); `-M` omits the OLDEST ancestors, `-C` keeps the FIRST children by PID.
- New flag = new clap field → `RenderOptions` field → render unit test → `.toml` fixture → `docs/cli.md` flags table. All five, or the change is incomplete.
