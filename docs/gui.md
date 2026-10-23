# `phg` — GUI reference

`phg` is the gpui/gpui-kit desktop frontend over the same `prochist-core`
model as `ph` and `phi`. It opens a native window (Windows/Linux) with three
panes: a main view on top, a details pane, and a status bar, plus a toolbar
with the search box and action buttons. The theme follows the OS
(light/dark).

```sh
phg                 # all processes (sortable table)
phg -p PID          # collapsible process tree around PID
phg -f FILE|DIR     # processes holding FILE/DIR open (same rules as ph)
```

`-p` and `-f` conflict; errors print `phg: error: ...` to stderr and exit 1
before any window opens. The hidden `--snapshot <FILE>` flag replays a JSON
fixture instead of querying the OS (testing only, same as `ph`/`phi`).

## Views

- **Processes** (no arguments): all processes in a virtualized, sortable
  table (PID, Name, Command, Executable, Service). Click a column header to
  sort. Typing in the search box live-filters the rows.
- **Tree** (`-p PID` or after focusing a process): the full descendant
  hierarchy of the outermost ancestor — collapsible/expandable. The focused
  process is highlighted.
- **Holders** (`-f PATH`): processes holding the path, same table as the
  processes view.

The details pane shows the selection's PID, PPID, Name, Command, Executable
and Service fields; the status bar shows the view tag, `index/total`, status
messages, and the focus breadcrumb.

## Mouse

| Action | Effect |
| --- | --- |
| click a row | select it |
| double-click a row | focus that process (re-root the tree) |
| right-click a row | action menu: focus, copy name/command/PID/path |
| click a column header | sort ascending/descending/off |
| click a details field | copy the field value |
| toolbar buttons | All Processes / Back (restore previous focus) / search box / Prev / Next (search matches, tree & holders views only) / Refresh / ? Help |

## Keyboard

Main view:

| Key | Effect |
| --- | --- |
| `j` / `k` | move down / up one row |
| `Ctrl-d` / `Ctrl-u` | half page down / up |
| `Ctrl-f` / `Ctrl-b` | full page down / up |
| `gg` / `G` | first / last row |
| arrows, `Left` / `Right` | move; collapse / expand tree nodes |
| `Enter` / `f` | focus the selected process (table views) |
| `f` | focus the selected process (tree view) |
| `Enter` (tree) | expand / collapse a node |
| `Backspace` | return to the previous focus |
| `Esc` (tree) | return to the previous focus |
| `Esc` (table) | clear the row selection |
| `/` | focus the search box |
| `n` / `N` | next / previous search match (tree/holders) |
| `Tab` / `i` | focus the details pane |
| `y` / `Y` | copy "name (pid)" / the command line |
| `r` | refresh the process snapshot |
| `?` | toggle help |
| `q` | quit |

Details pane: `j`/`k` move between fields, `y` copies the field value,
`Y` copies "name (pid)", `Tab`/`Esc`/`Enter` return to the main view.

Search box: `Enter` applies the query (live filter in the processes view;
jump to the first match in tree/holders), `Esc` cancels and restores the
previous filter.

Search syntax matches the TUI: case-insensitive substring, field prefixes
`pid:` `name:` `cmd:` `exe:` `service:`, no prefix = name + command.

## Implementation notes

- `src/state.rs` — pure logic ported from the TUI: display/search helpers,
  `build_subtree` (full descendant tree), expansion flattening, match
  finding. Unit-tested here; it is deliberately gpui-free.
- `src/app.rs` — the root view: entity wiring (Tree/DataTable/Input), focus
  stack, background provider roundtrips (`cx.spawn_in` + background
  executor), actions.
- `src/table.rs` — the shared sortable `TableDelegate` behind the processes
  and holders views.
- Provider calls run off the UI thread; selection (by PID), the processes
  filter and tree expansion are preserved across refreshes.
- UI integration tests (`src/ui_tests.rs`) use `gpui-kit/test-support` with
  fixture data through the same `Source::Fixture` path as `--snapshot`.
