# prochist

`ph` — print the process tree around a PID.

Windows is the shipping target; Linux is the development/verification platform.

```
$ ph 300
├── init (1)
├── login (100)
├── bash (200)
└── ph (300)
    ├── worker (301)
    └── logger (302)
```

Ancestors are listed root-first, the target process closes the list with `└──`,
and its direct children are nested below it.

## Install / run

```sh
cargo run --bin ph -- [flags] [PID]
```

`[PID]` defaults to the PID of the `ph` process itself.

## Flags

| Flag | Effect |
| --- | --- |
| `-A`, `--ascii` | Use ASCII connectors instead of Unicode box-drawing glyphs. |
| `-L`, `--long` | Show the full command line under each process. |
| `-E`, `--executable` | Show the full executable path instead of the binary name. |
| `-M`, `--max-ancestors <N>` | Show at most `N` ancestors (nearest to the target). |
| `-C`, `--max-children <N>` | Show at most `N` children (first by PID). |

Full CLI reference: [docs/cli.md](docs/cli.md).

## Exit codes

- `0` — tree printed.
- `1` — unknown PID or unreadable process snapshot.

## Development

Rust workspace, edition 2024:

```
crates/prochist-core   # data model, ProcessProvider trait, tree building, platform backends
bin/prochist-cli       # the `ph` binary (clap 4 derive) + tree rendering
docs/                  # design notes (architecture, CLI, Windows plan)
```

```sh
cargo test          # full suite (unit + trycmd end-to-end)
cargo clippy
cargo fmt --check
```

Design details: [docs/architecture.md](docs/architecture.md),
[docs/windows-notes.md](docs/windows-notes.md).

## License

MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
