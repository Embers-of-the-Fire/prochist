use std::io;
use std::path::Path;

use prochist_core::{MockProvider, Pid, ProcessInfo, ProcessProvider, default_provider};

/// Where process data comes from. `Live` talks to the OS; `Fixture` replays a
/// `--snapshot` JSON file. Cloning is cheap enough for background tasks
/// (`Fixture` clones its process list).
#[derive(Clone)]
pub enum Source {
    Live,
    Fixture(Vec<ProcessInfo>),
}

impl Source {
    pub fn snapshot(&self) -> io::Result<Vec<ProcessInfo>> {
        match self {
            Source::Live => default_provider().snapshot(),
            Source::Fixture(processes) => Ok(processes.clone()),
        }
    }

    pub fn holders(&self, path: &Path) -> io::Result<Vec<Pid>> {
        match self {
            Source::Live => default_provider().holders(path),
            Source::Fixture(processes) => MockProvider::new(processes.clone()).holders(path),
        }
    }
}
