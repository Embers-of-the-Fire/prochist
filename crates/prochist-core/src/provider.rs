use std::io;
use std::path::Path;

use crate::model::{Pid, ProcessInfo};

pub trait ProcessProvider {
    fn snapshot(&self) -> io::Result<Vec<ProcessInfo>>;

    fn holders(&self, _path: &Path) -> io::Result<Vec<Pid>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "file-holder queries are not supported by this provider",
        ))
    }
}
