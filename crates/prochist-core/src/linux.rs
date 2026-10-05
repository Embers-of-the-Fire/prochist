use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::model::{Pid, ProcessInfo};
use crate::provider::ProcessProvider;

pub struct LinuxProvider;

impl ProcessProvider for LinuxProvider {
    fn holders(&self, path: &Path) -> io::Result<Vec<Pid>> {
        let canonical = fs::canonicalize(path)?;
        let target_meta = fs::metadata(&canonical)?;
        let mut pids = Vec::new();
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            let Ok(pid) = name.parse::<Pid>() else {
                continue;
            };
            let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
                continue;
            };
            let holds = fds
                .flatten()
                .any(|fd| fd_matches(&fd.path(), &canonical, &target_meta));
            if holds {
                pids.push(pid);
            }
        }
        pids.sort_unstable();
        Ok(pids)
    }

    fn snapshot(&self) -> io::Result<Vec<ProcessInfo>> {
        let mut processes = Vec::new();
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            let Ok(pid) = name.parse::<u32>() else {
                continue;
            };
            match parse_stat(pid, &entry.path().join("stat")) {
                Ok(mut info) => {
                    info.command = read_command(&entry.path());
                    info.exe = read_exe(&entry.path());
                    processes.push(info);
                }
                Err(_) => continue,
            }
        }
        Ok(processes)
    }
}

fn fd_matches(fd_path: &Path, canonical: &Path, target_meta: &fs::Metadata) -> bool {
    if target_meta.is_dir() {
        let Ok(target) = fs::read_link(fd_path) else {
            return false;
        };
        match fs::canonicalize(target) {
            Ok(t) => t.starts_with(canonical),
            Err(_) => false,
        }
    } else {
        match fs::metadata(fd_path) {
            Ok(m) => m.dev() == target_meta.dev() && m.ino() == target_meta.ino(),
            Err(_) => false,
        }
    }
}

fn read_exe(proc_dir: &Path) -> Option<String> {
    fs::read_link(proc_dir.join("exe"))
        .ok()
        .map(|p| p.display().to_string())
}

fn read_command(proc_dir: &Path) -> Option<String> {
    let raw = fs::read(proc_dir.join("cmdline")).ok()?;
    let cmdline = raw
        .split(|b| *b == 0)
        .filter(|arg| !arg.is_empty())
        .map(|arg| String::from_utf8_lossy(arg).into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    if cmdline.is_empty() {
        None
    } else {
        Some(cmdline)
    }
}

fn parse_stat(pid: u32, path: &Path) -> io::Result<ProcessInfo> {
    let raw = fs::read_to_string(path)?;
    let open = raw
        .find('(')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed stat"))?;
    let close = raw
        .rfind(')')
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed stat"))?;
    let name = raw[open + 1..close].to_string();
    let ppid = raw[close + 2..]
        .split_whitespace()
        .nth(1)
        .and_then(|f| f.parse::<u32>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing ppid"))?;
    Ok(ProcessInfo {
        pid,
        ppid,
        name,
        command: None,
        exe: None,
        open_files: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_contains_current_process() {
        if !Path::new("/proc").exists() {
            return;
        }
        let snapshot = LinuxProvider.snapshot().unwrap();
        let own = std::process::id();
        assert!(snapshot.iter().any(|p| p.pid == own));
    }

    #[test]
    fn parses_stat_with_spaces_in_comm() {
        let dir = std::env::temp_dir().join(format!("prochist-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let stat = dir.join("stat");
        fs::write(
            &stat,
            "1234 (my weird proc) S 100 1234 1234 0 -1 4194304 100",
        )
        .unwrap();
        let info = parse_stat(1234, &stat).unwrap();
        assert_eq!(info.name, "my weird proc");
        assert_eq!(info.ppid, 100);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn holders_finds_process_with_open_file() {
        if !Path::new("/proc").exists() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("prochist-test-file-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("held.txt");
        fs::write(&file, b"held").unwrap();
        let _open = fs::File::open(&file).unwrap();
        let pids = LinuxProvider.holders(&file).unwrap();
        assert!(pids.contains(&std::process::id()));
        drop(_open);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn holders_matches_files_under_directory() {
        if !Path::new("/proc").exists() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("prochist-test-dir-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("held.txt");
        fs::write(&file, b"held").unwrap();
        let _open = fs::File::open(&file).unwrap();
        let pids = LinuxProvider.holders(&dir).unwrap();
        assert!(pids.contains(&std::process::id()));
        drop(_open);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn holders_errors_on_missing_path() {
        let missing = Path::new("/proc/definitely-not-a-real-path-xyz");
        assert!(LinuxProvider.holders(missing).is_err());
    }
}
