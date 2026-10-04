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
    ├── worker (301)
    └── logger (302)
```

Ancestors are listed flat, the current process closes the list with `└──`,
and its children are nested below. By default each node shows the binary name
(basename of the executable path, never truncated) and its PID.

## Flags

| Flag | Effect |
| --- | --- |
| `-A`, `--ascii` | Use ASCII connectors (`\|--`, `+--`, `\|`) instead of Unicode box-drawing glyphs. |
| `-L`, `--long` | Show the full command line on a continuation line under each process. |
| `-E`, `--executable` | Show the full executable path instead of the binary name. |

```
$ ph -L 300
├── init (1)
│   /sbin/init splash
├── bash (200)
│   /usr/bin/bash -l
└── ph (300)
    /usr/local/bin/ph 300
    ├── worker (301)
    │   /usr/bin/worker --daemon
    └── logger (302)
```

## Arguments

- `[PID]` — process ID to inspect. Defaults to the PID of the `ph` process itself.

## Exit codes

- `0` — tree printed.
- `1` — the PID does not exist, or the process snapshot could not be read.

## Testing flag (hidden)

- `--snapshot <FILE.json>` — replaces the OS process table with a mocked
  snapshot loaded from a JSON array of `{"pid", "ppid", "name"}` objects.
  Used by the end-to-end test suite; hidden from `--help`.
