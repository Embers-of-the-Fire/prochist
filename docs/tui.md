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
- **Status bar** (bottom line): mode tag (`TREE`/`DETAIL`/`ACTIONS`/`HELP`),
  cursor position, focus breadcrumb when re-rooted (e.g. `ph > worker`), and
  transient messages (yank confirmations, refresh errors).

## Focus mode

`Enter` (or `f`) on a tree row re-roots the whole view around that process —
the same tree `ph <pid>` would print. `Esc` / `Backspace` walks back up the
focus stack to the previously focused process. The status bar shows a
breadcrumb of the focus chain, and `r` refreshes around the *focused* PID, not
the startup one. Focusing a process that has just exited shows an error in the
status bar and leaves the view unchanged.

## Action menu

`a` opens a popup over the selected row with per-process actions. Move with
`j`/`k` and run with `Enter`, or press the action's hotkey directly. `Esc`
(or `a`/`q`) closes it.

| Hotkey | Action |
| --- | --- |
| `f` | focus this process (same as `Enter` on the row) |
| `y` | yank `name (pid)` |
| `c` | yank the command line |
| `p` | yank the PID |
| `e` | yank the executable path |

## Key bindings

| Key | Context | Action |
| --- | --- | --- |
| `j` / `Down` | anywhere | move down one row/field |
| `k` / `Up` | anywhere | move up one row/field |
| `Ctrl-d` / `Ctrl-u` | tree | half page down/up |
| `Ctrl-f` / `Ctrl-b` | tree | full page down/up |
| `gg` / `G` | tree | first / last row |
| `Home` / `End` | tree | first / last row |
| `Enter` / `f` | tree | focus the selected process (re-root the tree) |
| `Esc` / `Backspace` | tree | return to the previous focus |
| `a` | tree | open the action menu for the selected row |
| `Tab` / `i` | tree | focus the details pane |
| `Esc` / `q` | details | back to the tree pane |
| `y` | tree | yank `name (pid)` of the selected row |
| `Y` | tree | yank the full command line (falls back to exe path) |
| `y` | details | yank the selected field value |
| `Y` | details | yank `name (pid)` |
| `r` | tree | refresh the process snapshot (around the focused PID) |
| `?` | anywhere | toggle the help screen (scrollable with `j`/`k`) |
| `q` | tree | quit |

## Clipboard

Yanking emits an OSC 52 escape sequence, so no clipboard library or platform
dependency is needed. It works in terminals that support OSC 52: kitty,
alacritty, wezterm, foot, iTerm2, Windows Terminal, and tmux (3.3+; needs
`set -g allow-passthrough on` for nested sessions). VTE-based terminals
(GNOME Terminal, Konsole by default) ignore the sequence — yanks silently
no-op there.
