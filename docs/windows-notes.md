# Windows backend notes

Windows is the shipping target for `ph`. The backend (`WindowsProvider` in
`crates/prochist-core/src/windows.rs`) uses a Toolhelp snapshot:

1. `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)`
2. Iterate with `Process32FirstW` / `Process32NextW`
3. Map `PROCESSENTRY32W { th32ProcessID, th32ParentProcessID, szExeFile }`
   into `ProcessInfo { pid, ppid, name }`

`exe` is populated best-effort via `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`
+ `QueryFullProcessImageNameW`; `command` stays `None` (not exposed by Toolhelp).

File-holder queries (`ph -f FILE`) use the Restart Manager API
(`RmStartSession` → `RmRegisterResources` → `RmGetList` → `RmEndSession` from
`rstrtmgr.dll`) rather than a full system handle scan: it is documented,
stable, and sufficient for "which processes have this file open". Trade-offs:

- **Files only**: Restart Manager registers file paths, not directories, so
  `ph -f DIR` fails with an error on Windows. The Linux backend supports
  directories via `/proc/*/fd` prefix matching.
- **No handle-scan hangs**: enumerating all system handles
  (`NtQuerySystemInformation`, handle.exe-style) can block on named pipes and
  needs elevation; Restart Manager avoids that entirely.

Development and CI verification happen on the Linux backend, which shares the
same `ProcessProvider` trait; CI additionally runs the full suite (including a
live-snapshot smoke test) on `windows-latest`.

## Caveats

- **PID reuse**: Windows reuses PIDs aggressively. A stored parent PID may now
  belong to an unrelated process, producing bogus ancestor chains. `build_tree`
  already guards against cycles, but a reused PID that does not form a cycle is
  undetectable from a single snapshot.
- **Exited parents**: a process whose parent has exited keeps the dead parent's
  PID. The ancestor walk must stop gracefully when the parent is absent from the
  snapshot (already handled).
- **Snapshot consistency**: Toolhelp snapshots are point-in-time; processes can
  exit during iteration. Entries that vanish mid-scan should be skipped.
- **Protected processes**: names of some system processes may be inaccessible
  without elevation; fall back to a placeholder name instead of failing the
  whole snapshot.
