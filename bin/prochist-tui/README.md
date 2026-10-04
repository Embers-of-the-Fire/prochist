# prochist-tui

`phi` — interactive terminal UI for the process tree around a PID, built on
the `prochist-core` model with ratatui/crossterm.

```
phi [PID]
```

Vim-like navigation (`j/k`, `Ctrl-d/u`, `gg/G`), a details pane with field
yanking (`y`/`Y`, via OSC 52), and a help screen (`?`). See
[`docs/tui.md`](../../docs/tui.md) for the full keymap.
