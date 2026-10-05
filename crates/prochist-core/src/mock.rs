use std::io;
use std::path::Path;

use crate::model::{Pid, ProcessInfo};
use crate::provider::ProcessProvider;

pub struct MockProvider {
    processes: Vec<ProcessInfo>,
}

impl MockProvider {
    pub fn new(processes: Vec<ProcessInfo>) -> Self {
        Self { processes }
    }

    pub fn from_json_str(json: &str) -> serde_json::Result<Self> {
        Ok(Self::new(serde_json::from_str(json)?))
    }

    pub fn from_json_file(path: &Path) -> io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        Self::from_json_str(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

impl ProcessProvider for MockProvider {
    fn snapshot(&self) -> io::Result<Vec<ProcessInfo>> {
        Ok(self.processes.clone())
    }

    fn holders(&self, path: &Path) -> io::Result<Vec<Pid>> {
        let query = path.to_string_lossy();
        let dir_prefix = format!("{query}/");
        let mut pids: Vec<Pid> = self
            .processes
            .iter()
            .filter(|p| {
                p.open_files
                    .iter()
                    .any(|f| f == &query || f.starts_with(&dir_prefix))
            })
            .map(|p| p.pid)
            .collect();
        pids.sort_unstable();
        Ok(pids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_processes_from_json() {
        let json = r#"[{"pid": 1, "ppid": 0, "name": "init"}]"#;
        let provider = MockProvider::from_json_str(json).unwrap();
        let snapshot = provider.snapshot().unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].name, "init");
        assert!(snapshot[0].open_files.is_empty());
    }

    #[test]
    fn holders_matches_exact_file() {
        let json = r#"[
            {"pid": 1, "ppid": 0, "name": "a", "open_files": ["/var/log/app.log"]},
            {"pid": 2, "ppid": 0, "name": "b", "open_files": ["/var/log/other.log"]}
        ]"#;
        let provider = MockProvider::from_json_str(json).unwrap();
        assert_eq!(
            provider.holders(Path::new("/var/log/app.log")).unwrap(),
            vec![1]
        );
    }

    #[test]
    fn holders_matches_files_under_directory() {
        let json = r#"[
            {"pid": 1, "ppid": 0, "name": "a", "open_files": ["/srv/data/x.db"]},
            {"pid": 2, "ppid": 0, "name": "b", "open_files": ["/srv/database/y.db"]}
        ]"#;
        let provider = MockProvider::from_json_str(json).unwrap();
        assert_eq!(provider.holders(Path::new("/srv/data")).unwrap(), vec![1]);
    }

    #[test]
    fn holders_returns_empty_when_nothing_matches() {
        let json = r#"[{"pid": 1, "ppid": 0, "name": "a"}]"#;
        let provider = MockProvider::from_json_str(json).unwrap();
        assert!(provider.holders(Path::new("/nope")).unwrap().is_empty());
    }
}
