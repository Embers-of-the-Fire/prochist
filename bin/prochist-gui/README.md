# prochist-gui

`phg` — a gpui/gpui-kit desktop GUI for the process tree around a PID, and
for the processes holding a file or directory open.

```sh
phg                 # all processes (sortable table)
phg -p PID          # collapsible process tree around PID
phg -f FILE|DIR     # processes holding FILE/DIR open
                    # (Windows: files only, no directory queries)
```

Mouse-first (click to select, double-click to focus, right-click for copy
actions, click column headers to sort) with keyboard navigation kept from
the TUI (`j`/`k`, `/` search with `pid:`/`name:`/`cmd:`/`exe:`/`service:`
prefixes, `r` refresh, `?` help).

See the [workspace repository](https://github.com/y4nettechi/prochist) for
documentation (`docs/gui.md`) and the sibling tools `ph` (CLI) and `phi`
(TUI).
