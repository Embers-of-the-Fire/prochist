# phi — interactive TUI

`phi` (`bin/prochist-tui`) is a ratatui/crossterm frontend over the same
`ProcessTree` model as `ph`. It shows a scrollable tree pane on top and a
details pane for the selected process at the bottom.

```
phi [PID]
```

PID defaults to the current process. The hidden `--snapshot <FILE>` flag works
like in `ph` (MockProvider, testing only) and is handy for driving the TUI
against a fixed tree.

## Layout

- **Processes** (top, fills): the ancestor chain, the target process (cyan,
  bold), and its children — same shape as `ph` output, one row per process.
- **Details** (bottom, fixed 7 rows): PID, PPID, Name, Command, Executable of
  the selected row.
- **Status bar** (bottom line): mode tag (`TREE`/`DETAIL`/`HELP`), cursor
  position, transient messages (yank confirmations, refresh errors).

## Key bindings

| Key | Context | Action |
| --- | --- | --- |
| `j` / `Down` | anywhere | move down one row/field |
| `k` / `Up` | anywhere | move up one row/field |
| `Ctrl-d` / `Ctrl-u` | tree | half page down/up |
| `Ctrl-f` / `Ctrl-b` | tree | full page down/up |
| `gg` / `G` | tree | first / last row |
| `Home` / `End` | tree | first / last row |
| `Tab` / `i` / `Enter` | tree | focus the details pane |
| `Esc` / `q` | details | back to the tree pane |
| `y` | tree | yank `name (pid)` of the selected row |
| `Y` | tree | yank the full command line (falls back to exe path) |
| `y` | details | yank the selected field value |
| `Y` | details | yank `name (pid)` |
| `r` | tree | refresh the process snapshot |
| `?` | anywhere | toggle the help screen (scrollable with `j`/`k`) |
| `q` | tree | quit |

## Clipboard

Yanking emits an OSC 52 escape sequence, so no clipboard library or platform
dependency is needed. It works in terminals that support OSC 52: kitty,
alacritty, wezterm, foot, iTerm2, Windows Terminal, and tmux (3.3+; needs
`set -g allow-passthrough on` for nested sessions). VTE-based terminals
(GNOME Terminal, Konsole by default) ignore the sequence — yanks silently
no-op there.
