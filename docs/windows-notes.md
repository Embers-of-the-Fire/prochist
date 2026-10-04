# Windows backend notes

Windows is the shipping target for `ph`. The backend is currently stubbed
(`WindowsProvider::snapshot` is `todo!()`); development and verification happen
on the Linux backend, which shares the same `ProcessProvider` trait.

## Planned implementation

Use a Toolhelp snapshot:

1. `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)`
2. Iterate with `Process32FirstW` / `Process32NextW`
3. Map `PROCESSENTRY32W { th32ProcessID, th32ParentProcessID, szExeFile }`
   into `ProcessInfo { pid, ppid, name }`

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
