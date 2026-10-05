# ph — CLI

```
ph [flags] [-p PID]
ph [flags] -f FILE|DIR
```

`ph -p PID` prints the process tree around a single PID: the ancestor chain
above it (root first), the process itself, and its direct children below.

```
$ ph -p 300
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

`ph -f FILE` lists the processes that have a file open; `ph -f DIR` lists
processes holding any file under that directory. The path is printed as a
header line with the holding processes as a list below it.

```
$ ph -f /var/log/app.log
/var/log/app.log
├── vim (123)
└── code (456)
```

On Windows only files are supported (the backend uses Restart Manager, which
accepts file paths only); `ph -f DIR` exits 1 with an error.

## Flags

| Flag | Effect |
| --- | --- |
| `-p`, `--pid <PID>` | Process ID to inspect. Defaults to the PID of the `ph` process itself. Conflicts with `-f`. |
| `-f`, `--file <PATH>` | List processes that have `PATH` (file or directory) open instead of printing a tree. Conflicts with `-p` and `-M`. |
| `-A`, `--ascii` | Use ASCII connectors (`\|--`, `+--`, `\|`) instead of Unicode box-drawing glyphs. |
| `-L`, `--long` | Show the full command line on a continuation line under each process. |
| `-E`, `--executable` | Show the full executable path instead of the binary name. |
| `-M`, `--max-ancestors <N>` | Show at most `N` ancestors (the ones nearest to the target), replacing the omitted oldest with a `... x processes omitted` line. |
| `-C`, `--max-children <N>` | Show at most `N` children (or file holders, in `-f` mode), ending the list with a `... x processes omitted` line. |

```
$ ph -L -p 300
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

```
$ ph -M 2 -C 2 -p 300
├── ... 2 processes omitted
├── login (100)
├── bash (200)
└── ph (300)
    ├── worker (301)
    ├── logger (302)
    └── ... 2 processes omitted
```

## Exit codes

- `0` — tree or holder list printed.
- `1` — the PID does not exist, no process has the path open, the path is
  unreadable, the query is unsupported (e.g. `-f DIR` on Windows), or the
  process snapshot could not be read.

## Testing flag (hidden)

- `--snapshot <FILE.json>` — replaces the OS process table with a mocked
  snapshot loaded from a JSON array of `{"pid", "ppid", "name"}` objects
  (optional fields: `command`, `exe`, `open_files`). `ph -f` queries against
  a mock are answered from each process's `open_files` list. Used by the
  end-to-end test suite; hidden from `--help`.
