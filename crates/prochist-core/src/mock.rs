use std::io;
use std::path::Path;

use crate::model::ProcessInfo;
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
    }
}
