# prochist

`ph` — print the process tree around a PID. `phi` — explore it interactively.

Windows is the shipping target; Linux is the development/verification platform.

```
$ ph -p 300
├── init (1)
├── login (100)
├── bash (200)
└── ph (300)
    ├── worker (301)
    └── logger (302)
```

Ancestors are listed root-first, the target process closes the list with `└──`,
and its direct children are nested below it.

`ph -f FILE` answers the reverse question — which processes have a file (or a
file under a directory) open:

```
$ ph -f /var/log/app.log
/var/log/app.log
├── vim (123)
└── code (456)
```

## Install / run

```sh
cargo run --bin ph -- [flags] [-p PID]
cargo run --bin ph -- [flags] -f FILE|DIR
```

Without `-p`, `ph` inspects its own PID. `-f DIR` is Linux-only for now
(the Windows backend uses Restart Manager, which accepts files only).

## Flags

| Flag | Effect |
| --- | --- |
| `-p`, `--pid <PID>` | Process ID to inspect (default: the `ph` process itself). |
| `-f`, `--file <PATH>` | List processes that have `PATH` (file or directory) open. |
| `-A`, `--ascii` | Use ASCII connectors instead of Unicode box-drawing glyphs. |
| `-L`, `--long` | Show the full command line under each process. |
| `-E`, `--executable` | Show the full executable path instead of the binary name. |
| `-M`, `--max-ancestors <N>` | Show at most `N` ancestors (nearest to the target). |
| `-C`, `--max-children <N>` | Show at most `N` children or file holders (first by PID). |

Full CLI reference: [docs/cli.md](docs/cli.md).

## `phi` — interactive TUI

`phi` is a ratatui/crossterm frontend over the same process tree:

```
┌ Processes ──────────────┐
│ ├── init (1)            │
│ ├── bash (200)          │
│ └── ph (300)            │
│     ├── worker (301)    │
├ Details ────────────────┤
│ PID        : 300        │
│ Command    : ph 300     │
└ [TREE] 3/4 │ ?: help ───┘
```

```sh
cargo run --bin phi -- [-p PID]
cargo run --bin phi -- -f FILE|DIR
```

`phi -f PATH` opens the same holder list as `ph -f`, interactively: pick a
process and press `Enter` to dive into its tree, `Esc` to come back.

Vim-like navigation (`j`/`k`, `Ctrl-d`/`u`, `gg`/`G`), a details pane with
field-wise yanking (`y`/`Y`, via OSC 52), and a help screen (`?`). Full keymap:
[docs/tui.md](docs/tui.md).

## Exit codes

- `0` — tree or holder list printed.
- `1` — unknown PID, no holders for a `-f` path, unsupported query, or
  unreadable process snapshot.

## Development

Rust workspace, edition 2024:

```
crates/prochist-core   # data model, ProcessProvider trait, tree building, platform backends
bin/prochist-cli       # the `ph` binary (clap 4 derive) + tree rendering
bin/prochist-tui       # the `phi` binary (ratatui/crossterm TUI)
docs/                  # design notes (architecture, CLI, TUI, Windows plan)
```

```sh
cargo test          # full suite (unit + trycmd end-to-end)
cargo clippy
cargo fmt --check
```

Design details: [docs/architecture.md](docs/architecture.md),
[docs/tui.md](docs/tui.md), [docs/windows-notes.md](docs/windows-notes.md).

## License

MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
