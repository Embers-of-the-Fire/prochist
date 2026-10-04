# ph — CLI

```
ph [PID]
```

Prints the process tree around a single PID: the ancestor chain above it
(root first), the process itself, and its direct children below.

```
$ ph 300
├── init (1)
├── login (100)
├── bash (200)
└── ph (300)
    target/debug/ph 300
    ├── worker (301)
    │   /usr/bin/worker --daemon
    └── logger (302)
```

Ancestors are listed flat, the current process closes the list with `└──`,
and its children are nested below. The connector line shows the process name
and PID; the full command line (never truncated) is shown on a continuation
line under each node when available.

## Arguments

- `[PID]` — process ID to inspect. Defaults to the PID of the `ph` process itself.

## Exit codes

- `0` — tree printed.
- `1` — the PID does not exist, or the process snapshot could not be read.

## Testing flag (hidden)

- `--snapshot <FILE.json>` — replaces the OS process table with a mocked
  snapshot loaded from a JSON array of `{"pid", "ppid", "name"}` objects.
  Used by the end-to-end test suite; hidden from `--help`.
