# phi — interactive TUI

`phi` (`bin/prochist-tui`) is a ratatui/crossterm frontend over the same
`ProcessTree` model as `ph`. It shows a scrollable tree pane on top and a
details pane for the selected process at the bottom.

```
phi
phi -p PID
phi -f FILE|DIR
```

With no arguments, `phi` opens the processes view: a flat, searchable list of
all running processes. With `-p PID` it opens the tree around that PID. The
hidden `--snapshot <FILE>` flag works like in `ph` (MockProvider, testing only)
and is handy for driving the TUI against a fixed tree.

`phi -f PATH` starts in the holders view: the processes that have the file (or
a file under the directory) open, listed under the queried path. If no process
holds the path, `phi` exits 1 like `ph`. As with `ph`, `-f DIR` is Linux-only
(the Windows backend uses Restart Manager, which accepts files only).

## Layout

- **Processes** (top, fills): the ancestor chain, the target process (cyan,
  bold), and its children — same shape as `ph` output, one row per process.
- **Details** (bottom, fixed 7 rows): PID, PPID, Name, Command, Executable of
  the selected row.
- **Status bar** (bottom line): mode tag
  (`TREE`/`HOLDERS`/`PROCESSES`/`DETAIL`/`ACTIONS`/`SEARCH`/`HELP`), cursor
  position, focus breadcrumb when re-rooted (e.g. `ph > worker`), the search
  prompt while searching, and transient messages (yank confirmations, refresh
  errors).

## Processes view

Started with plain `phi`. The top pane lists every process one per row as
`pid name — command`, sorted by PID. `/` opens a live filter: each keystroke
narrows the list, `Enter` keeps the filter, `Esc` cancels the edit (restoring
the previous filter); `Esc` outside the search prompt clears the active
filter. `Enter` (or `f`) on a process re-roots into its tree, and `Esc` /
`Backspace` from there returns to the (cached, still-filtered) list. `r`
re-snapshots the process table, preserving the filter and the selected PID.

## Search

`/` searches in every view. In the processes view it live-filters the full
process list; in the tree and holders views it jumps to the first match on
`Enter`, and `n` / `N` cycle through the remaining matches. Matching is a
case-insensitive substring over the process name and command line; a field
prefix narrows it: `pid:300` (PID prefix), `name:vim`, `cmd:--daemon`,
`exe:/usr/bin`.

## Holders view

Started with `phi -f PATH`. The top pane lists the holding processes under the
queried path (`Holders of <path>`); navigation, yanking, the details pane, and
the action menu work exactly as in the tree view. `Enter` (or `f`) on a holder
re-roots into that process's tree — the breadcrumb shows e.g.
`/var/log/app.log > vim` — and `Esc` / `Backspace` from there returns to the
(cached) holders list. `r` re-queries which processes hold the path; if none
remain, the list empties (position `0/0`) and a status note explains.

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
| `r` | holders | re-query which processes hold the path |
| `r` | processes | re-snapshot the process table |
| `/` | processes | live-filter all processes |
| `/` | tree / holders | jump-to-match search (`Enter` applies, `Esc` cancels) |
| `n` / `N` | tree / holders | next / previous search match |
| `Esc` | processes | clear the active filter |
| `?` | anywhere | toggle the help screen (scrollable with `j`/`k`) |
| `q` | tree | quit |

## Clipboard

Yanking emits an OSC 52 escape sequence, so no clipboard library or platform
dependency is needed. It works in terminals that support OSC 52: kitty,
alacritty, wezterm, foot, iTerm2, Windows Terminal, and tmux (3.3+; needs
`set -g allow-passthrough on` for nested sessions). VTE-based terminals
(GNOME Terminal, Konsole by default) ignore the sequence — yanks silently
no-op there.
