//! Stable identities for target processes.

/// Opaque identity of one operating-system process instance.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProcessInstance {
    pid: u32,
    start_marker: u64,
}

impl ProcessInstance {
    pub(crate) const fn new(pid: u32, start_marker: u64) -> Self {
        Self { pid, start_marker }
    }

    /// Resolves the current operating-system identity for a process identifier.
    ///
    /// Returns `None` when the process does not exist or its creation marker is unavailable.
    #[must_use]
    pub fn for_pid(pid: u32) -> Option<Self> {
        process_start_marker(pid).map(|start_marker| Self::new(pid, start_marker))
    }

    /// Returns the operating-system process identifier.
    #[must_use]
    pub const fn pid(self) -> u32 {
        self.pid
    }

    /// Returns the platform-specific process creation marker.
    #[must_use]
    pub const fn start_marker(self) -> u64 {
        self.start_marker
    }

    /// Returns whether this identity still describes a live process instance.
    #[must_use]
    pub fn is_current(self) -> bool {
        process_start_marker(self.pid) == Some(self.start_marker)
    }
}

/// One supported target discovered through memflow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Target {
    instance: ProcessInstance,
    executable: String,
}

impl Target {
    pub(crate) fn new(instance: ProcessInstance, executable: String) -> Self {
        Self {
            instance,
            executable,
        }
    }

    /// Returns the exact operating-system process instance.
    #[must_use]
    pub const fn instance(&self) -> ProcessInstance {
        self.instance
    }

    /// Returns the supported executable name found during discovery.
    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }
}

#[cfg(target_os = "linux")]
fn process_start_marker(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, fields) = stat.rsplit_once(") ")?;
    fields.split_whitespace().nth(19)?.parse().ok()
}

#[cfg(not(target_os = "linux"))]
fn process_start_marker(pid: u32) -> Option<u64> {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );
    system.process(pid).map(sysinfo::Process::start_time)
}
