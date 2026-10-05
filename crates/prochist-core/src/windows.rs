//! Windows backend — the shipping target platform.
//! Toolhelp snapshot: CreateToolhelp32Snapshot + Process32FirstW/Process32NextW.
//! See docs/windows-notes.md for PID-reuse caveats.

use std::io;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_NO_MORE_FILES, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};

use crate::model::ProcessInfo;
use crate::provider::ProcessProvider;

pub struct WindowsProvider;

impl ProcessProvider for WindowsProvider {
    fn snapshot(&self) -> io::Result<Vec<ProcessInfo>> {
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if raw == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let snapshot = SnapshotHandle(raw);

        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        let mut processes = Vec::new();
        let first = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        if first == 0 {
            return check_enum_end(processes);
        }
        loop {
            processes.push(ProcessInfo {
                pid: entry.th32ProcessID,
                ppid: entry.th32ParentProcessID,
                name: exe_name(&entry),
                command: None,
                exe: query_exe_path(entry.th32ProcessID),
            });
            if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
                return check_enum_end(processes);
            }
        }
    }
}

fn check_enum_end(processes: Vec<ProcessInfo>) -> io::Result<Vec<ProcessInfo>> {
    let err = unsafe { GetLastError() };
    if err == ERROR_NO_MORE_FILES {
        Ok(processes)
    } else {
        Err(io::Error::from_raw_os_error(err as i32))
    }
}

struct SnapshotHandle(HANDLE);

impl Drop for SnapshotHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn exe_name(entry: &PROCESSENTRY32W) -> String {
    let len = entry
        .szExeFile
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(entry.szExeFile.len());
    String::from_utf16_lossy(&entry.szExeFile[..len])
}

fn query_exe_path(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return None;
    }
    let result = query_full_process_image_name(process);
    unsafe { CloseHandle(process) };
    result
}

fn query_full_process_image_name(process: HANDLE) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size) };
    if ok == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..size as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_contains_current_process() {
        let snapshot = WindowsProvider.snapshot().unwrap();
        let own = std::process::id();
        assert!(snapshot.iter().any(|p| p.pid == own));
    }
}
