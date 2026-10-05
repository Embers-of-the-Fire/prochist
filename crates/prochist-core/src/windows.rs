//! Windows backend — the shipping target platform.
//! Toolhelp snapshot: CreateToolhelp32Snapshot + Process32FirstW/Process32NextW.
//! See docs/windows-notes.md for PID-reuse caveats.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_MORE_DATA, ERROR_NO_MORE_FILES, ERROR_SUCCESS, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::RestartManager::{
    CCH_RM_SESSION_KEY, RM_PROCESS_INFO, RmEndSession, RmGetList, RmRegisterResources,
    RmStartSession,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};

use crate::model::{Pid, ProcessInfo};
use crate::provider::ProcessProvider;

pub struct WindowsProvider;

impl WindowsProvider {
    fn holders_inner(path: &Path) -> io::Result<Vec<Pid>> {
        if std::fs::metadata(path)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "directory queries are not supported on Windows (Restart Manager accepts files only)",
            ));
        }

        let mut session: u32 = 0;
        let mut key = [0u16; (CCH_RM_SESSION_KEY + 1) as usize];
        let key_text = format!("{:032x}", std::process::id());
        for (dst, src) in key.iter_mut().zip(key_text.encode_utf16()) {
            *dst = src;
        }
        win32_err(unsafe { RmStartSession(&mut session, 0, key.as_mut_ptr()) })?;
        let session = RmSession(session);

        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        win32_err(unsafe {
            RmRegisterResources(
                session.0,
                1,
                &wide.as_ptr(),
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
            )
        })?;

        let mut needed: u32 = 0;
        let mut count: u32 = 0;
        let mut reasons: u32 = 0;
        let err = unsafe {
            RmGetList(
                session.0,
                &mut needed,
                &mut count,
                std::ptr::null_mut(),
                &mut reasons,
            )
        };
        if err == ERROR_SUCCESS {
            return Ok(Vec::new());
        }
        if err != ERROR_MORE_DATA {
            return Err(io::Error::from_raw_os_error(err as i32));
        }
        let mut infos: Vec<RM_PROCESS_INFO> =
            (0..needed).map(|_| RM_PROCESS_INFO::default()).collect();
        count = needed;
        win32_err(unsafe {
            RmGetList(
                session.0,
                &mut needed,
                &mut count,
                infos.as_mut_ptr(),
                &mut reasons,
            )
        })?;
        let mut pids: Vec<Pid> = infos[..count as usize]
            .iter()
            .map(|i| i.Process.dwProcessId)
            .collect();
        pids.sort_unstable();
        Ok(pids)
    }
}

impl ProcessProvider for WindowsProvider {
    fn holders(&self, path: &Path) -> io::Result<Vec<Pid>> {
        Self::holders_inner(path)
    }

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
                open_files: Vec::new(),
            });
            if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
                return check_enum_end(processes);
            }
        }
    }
}

struct RmSession(u32);

impl Drop for RmSession {
    fn drop(&mut self) {
        unsafe { RmEndSession(self.0) };
    }
}

fn win32_err(code: u32) -> io::Result<()> {
    if code == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code as i32))
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
