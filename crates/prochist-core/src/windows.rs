//! Windows backend — the shipping target platform.
//! Toolhelp snapshot: CreateToolhelp32Snapshot + Process32FirstW/Process32NextW.
//! See docs/windows-notes.md for PID-reuse caveats.

use std::collections::HashMap;
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
use windows_sys::Win32::System::Services::{
    CloseServiceHandle, ENUM_SERVICE_STATUS_PROCESSW, EnumServicesStatusExW, OpenSCManagerW,
    SC_ENUM_PROCESS_INFO, SC_HANDLE, SC_MANAGER_ENUMERATE_SERVICE, SERVICE_ACTIVE, SERVICE_WIN32,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};

use crate::model::{Pid, ProcessInfo, ProcessRole, ServiceEntry};
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
        let roles = service_roles();
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
                role: roles
                    .get(&entry.th32ProcessID)
                    .map(|services| ProcessRole::WindowsService {
                        services: services.clone(),
                    }),
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

struct ScmHandle(SC_HANDLE);

impl Drop for ScmHandle {
    fn drop(&mut self) {
        unsafe { CloseServiceHandle(self.0) };
    }
}

fn service_roles() -> HashMap<Pid, Vec<ServiceEntry>> {
    let mut map: HashMap<Pid, Vec<ServiceEntry>> = HashMap::new();
    let raw = unsafe {
        OpenSCManagerW(
            std::ptr::null(),
            std::ptr::null(),
            SC_MANAGER_ENUMERATE_SERVICE,
        )
    };
    if raw.is_null() {
        return map;
    }
    let scm = ScmHandle(raw);

    let mut needed: u32 = 0;
    let mut count: u32 = 0;
    let mut resume: u32 = 0;
    unsafe {
        EnumServicesStatusExW(
            scm.0,
            SC_ENUM_PROCESS_INFO,
            SERVICE_WIN32,
            SERVICE_ACTIVE,
            std::ptr::null_mut(),
            0,
            &mut needed,
            &mut count,
            &mut resume,
            std::ptr::null(),
        );
    }
    if needed == 0 {
        return map;
    }

    const ENTRY_SIZE: usize = std::mem::size_of::<ENUM_SERVICE_STATUS_PROCESSW>();
    let mut capacity = needed as usize + ENTRY_SIZE;
    // Vec<ENUM_SERVICE_STATUS_PROCESSW> gives us storage with the correct
    // alignment; a Vec<u8> could be misaligned for the returned struct array.
    let mut storage: Vec<ENUM_SERVICE_STATUS_PROCESSW> = Vec::new();
    for _ in 0..5 {
        storage.clear();
        storage.resize(
            capacity.div_ceil(ENTRY_SIZE),
            ENUM_SERVICE_STATUS_PROCESSW {
                lpServiceName: std::ptr::null_mut(),
                lpDisplayName: std::ptr::null_mut(),
                ServiceStatusProcess: unsafe { std::mem::zeroed() },
            },
        );
        resume = 0;
        count = 0;
        let ok = unsafe {
            EnumServicesStatusExW(
                scm.0,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_ACTIVE,
                storage.as_mut_ptr() as *mut u8,
                (storage.len() * ENTRY_SIZE) as u32,
                &mut needed,
                &mut count,
                &mut resume,
                std::ptr::null(),
            )
        };
        if ok != 0 {
            for entry in &storage[..count as usize] {
                let pid = entry.ServiceStatusProcess.dwProcessId;
                if pid == 0 {
                    continue;
                }
                map.entry(pid).or_default().push(ServiceEntry {
                    name: wide_string(entry.lpServiceName),
                    display_name: non_empty_wide_string(entry.lpDisplayName),
                });
            }
            return map;
        }
        // The active service list can grow after the size query; retry with
        // the newly reported size instead of dropping all service tags.
        if unsafe { GetLastError() } != ERROR_MORE_DATA || needed as usize <= capacity {
            return map;
        }
        capacity = needed as usize + ENTRY_SIZE;
    }
    map
}

fn wide_string(ptr: windows_sys::core::PWSTR) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(ptr, len) })
}

fn non_empty_wide_string(ptr: windows_sys::core::PWSTR) -> Option<String> {
    let s = wide_string(ptr);
    if s.is_empty() { None } else { Some(s) }
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
