mod mock;
mod model;
mod provider;
mod tree;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(windows)]
mod windows;

pub use mock::MockProvider;
pub use model::{Pid, ProcessInfo, ProcessTree};
pub use provider::ProcessProvider;
pub use tree::{TreeError, build_tree};

#[cfg(target_os = "linux")]
pub use linux::LinuxProvider;

#[cfg(windows)]
pub use windows::WindowsProvider;

#[cfg(target_os = "linux")]
pub fn default_provider() -> Box<dyn ProcessProvider> {
    Box::new(LinuxProvider)
}

#[cfg(windows)]
pub fn default_provider() -> Box<dyn ProcessProvider> {
    Box::new(WindowsProvider)
}

#[cfg(not(any(target_os = "linux", windows)))]
compile_error!("prochist currently targets Windows; Linux is supported for development only");
