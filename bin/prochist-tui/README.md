# prochist-tui (`phi`)

The `phi` binary — an interactive terminal UI for the process tree around a
PID, built with ratatui/crossterm on top of the
[`prochist-core`](../../crates/prochist-core) model.

## Usage

```
phi [PID]
```

`[PID]` defaults to the PID of the `phi` process itself.

The screen is split into a scrollable **Processes** pane (the same tree shape
as `ph` output), a **Details** pane for the selected process, and a status bar.

| Key | Context | Action |
| --- | --- | --- |
| `j` / `k`, arrows | anywhere | move down / up |
| `Ctrl-d` / `Ctrl-u` | tree | half page down / up |
| `Ctrl-f` / `Ctrl-b` | tree | full page down / up |
| `gg` / `G` | tree | first / last row |
| `Tab` / `i` / `Enter` | tree | focus the details pane |
| `Esc` / `q` | details | back to the tree pane |
| `y` / `Y` | tree | yank `name (pid)` / full command line |
| `y` / `Y` | details | yank the selected field / `name (pid)` |
| `r` | tree | refresh the process snapshot |
| `?` | anywhere | toggle the help screen |
| `q` | tree | quit |

Yanking uses the OSC 52 escape sequence (no clipboard dependency, works over
SSH); terminals without OSC 52 support silently ignore it. Full reference:
[`docs/tui.md`](../../docs/tui.md).

## Structure

- `src/main.rs` — clap, provider selection (including the hidden
  `--snapshot <FILE>` testing flag shared with `ph`), terminal lifecycle, event
  loop. Thin: keys go into `App::handle_key`, effects come out.
- `src/app.rs` — pure application state and the vim keymap; `handle_key`
  returns `Option<Effect>` instead of doing I/O, so the whole keymap is
  unit-tested.
- `src/ui.rs` — ratatui rendering, tested with `TestBackend`.
- `src/clipboard.rs` — OSC 52 yank with an in-crate base64 encoder.

## Testing

```sh
cargo test -p prochist-tui                          # unit tests
cargo run --bin phi -- [PID]                        # run against the live OS
cargo run --bin phi -- --snapshot <FILE.json>       # run against a fixture
```

## License

MIT OR Apache-2.0.
