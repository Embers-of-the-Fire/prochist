use serde::{Deserialize, Serialize};

pub type Pid = u32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceEntry {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProcessRole {
    WindowsService { services: Vec<ServiceEntry> },
    SystemdService { unit: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: Pid,
    pub ppid: Pid,
    pub name: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub exe: Option<String>,
    #[serde(default)]
    pub open_files: Vec<String>,
    #[serde(default)]
    pub role: Option<ProcessRole>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessTree {
    pub ancestors: Vec<ProcessInfo>,
    pub current: ProcessInfo,
    pub children: Vec<ProcessInfo>,
}
