# prochist-cli (`ph`)

The `ph` binary — prints the process tree around a PID. Owns all user-facing
output; the model and platform logic live in
[`prochist-core`](../../crates/prochist-core).

## Usage

```
ph [flags] [PID]
```

`[PID]` defaults to the PID of the `ph` process itself.

| Flag | Effect |
| --- | --- |
| `-A`, `--ascii` | Use ASCII connectors instead of Unicode box-drawing glyphs. |
| `-L`, `--long` | Show the full command line under each process. |
| `-E`, `--executable` | Show the full executable path instead of the binary name. |
| `-M`, `--max-ancestors <N>` | Show at most `N` ancestors (nearest to the target). |
| `-C`, `--max-children <N>` | Show at most `N` children (first by PID). |

Full reference: [`docs/cli.md`](../../docs/cli.md).

## Structure

- `src/main.rs` — clap 4 derive CLI, provider selection, error → exit-code
  mapping. Kept thin; formatting decisions live in `render.rs`.
- `src/render.rs` — pure `render(&ProcessTree, &RenderOptions) -> String`,
  unit-tested with exact-string assertions.
- `tests/cli.rs` — trycmd runner over `tests/fixtures/*.toml`.
- `tests/fixtures/` — one `.toml` case per behavior, each fed by a
  `*.snapshot.json` via the hidden `ph --snapshot <FILE>` flag (swaps the OS
  provider for `MockProvider`; never shown in `--help`).

## Testing

```sh
cargo test -p prochist-cli                      # trycmd end-to-end suite
TRYCMD=overwrite cargo test -p prochist-cli     # bless fixtures after an intentional output change
cargo run --bin ph -- [flags] [PID]             # run against the live OS (Linux backend)
```

## License

MIT OR Apache-2.0.
