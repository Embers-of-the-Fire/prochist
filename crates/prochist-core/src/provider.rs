use std::io;

use crate::model::ProcessInfo;

pub trait ProcessProvider {
    fn snapshot(&self) -> io::Result<Vec<ProcessInfo>>;
}
