//! Windows backend — the shipping target platform.
//! Planned implementation: CreateToolhelp32Snapshot + Process32First/Process32Next.
//! See docs/windows-notes.md for PPID caveats.

use std::io;

use crate::model::ProcessInfo;
use crate::provider::ProcessProvider;

pub struct WindowsProvider;

impl ProcessProvider for WindowsProvider {
    fn snapshot(&self) -> io::Result<Vec<ProcessInfo>> {
        todo!("windows backend: CreateToolhelp32Snapshot (see docs/windows-notes.md)")
    }
}
