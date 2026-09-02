//! Native target discovery and read-only attachment.

use memflow::prelude::v1::{
    Address, MemoryView, Os, OsArgs, PartialResultExt, Process, ProcessInfo,
};
use memflow_native::{NativeOs, NativeProcess};

use crate::{AccessError, DiscoveryError, ProcessInstance, Target};

const TARGET_EXECUTABLES: &[&str] = &["Warframe.x64.exe", "Warframe.exe"];
#[cfg(target_os = "linux")]
const TRUNCATED_TARGET_EXECUTABLES: &[&str] = &["Warframe.x64.ex"];

/// Evidence that the target's executable mapping was readable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessProof {
    executable_mapping: String,
    probe_address: u64,
}

impl AccessProof {
    fn new(executable_mapping: String, probe_address: u64) -> Self {
        Self {
            executable_mapping,
            probe_address,
        }
    }

    /// Returns the mapped executable module used for the probe.
    #[must_use]
    pub fn executable_mapping(&self) -> &str {
        &self.executable_mapping
    }

    /// Returns the address of the byte used to prove read access.
    #[must_use]
    pub const fn probe_address(&self) -> u64 {
        self.probe_address
    }
}

/// A retained, read-only memflow attachment to one process instance.
pub struct AttachedTarget {
    target: Target,
    proof: AccessProof,
    process: NativeProcess,
}

impl AttachedTarget {
    fn new(target: Target, proof: AccessProof, process: NativeProcess) -> Self {
        Self {
            target,
            proof,
            process,
        }
    }

    /// Returns the discovered target metadata.
    #[must_use]
    pub const fn target(&self) -> &Target {
        &self.target
    }

    /// Returns the evidence retained from the initial access probe.
    #[must_use]
    pub const fn proof(&self) -> &AccessProof {
        &self.proof
    }

    /// Revalidates both the process instance and its readable executable mapping.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError::TargetChanged`] if the PID was reused or the process
    /// exited, and [`AccessError::Read`] if its executable mapping is no longer readable.
    pub fn verify(&mut self) -> Result<(), AccessError> {
        let instance = self.target.instance();
        ensure_current(instance)?;

        let mut probe = [0_u8; 1];
        self.process
            .read_raw_into(Address::from(self.proof.probe_address), &mut probe)
            .data()
            .map_err(|source| AccessError::Read {
                pid: instance.pid(),
                source,
            })?;

        ensure_current(instance)
    }
}

/// Discovers supported target processes using the same memflow view used for attachment.
///
/// Processes which exit before a stable creation marker can be captured are omitted.
///
/// # Errors
///
/// Returns an error if memflow-native cannot initialize or enumerate host processes.
pub fn discover_targets() -> Result<Vec<Target>, DiscoveryError> {
    let mut os = native_os()?;
    let mut targets = os
        .process_info_list()
        .map_err(DiscoveryError::Enumerate)?
        .into_iter()
        .filter_map(|process| target_from_process(&process))
        .collect::<Vec<_>>();
    targets.sort_by_key(|target| target.instance().pid());
    Ok(targets)
}

/// Opens a discovered target and proves that its executable mapping is readable.
///
/// The returned attachment retains the memflow process handle for subsequent reads and
/// target-liveness checks.
///
/// # Errors
///
/// Returns an error if the target exited, changed identity, cannot be opened, or cannot be read.
pub fn attach(target: &Target) -> Result<AttachedTarget, AccessError> {
    let expected = target.instance();
    ensure_current(expected)?;

    let mut os = native_os().map_err(AccessError::Discovery)?;
    let process_info = os
        .process_info_list()
        .map_err(DiscoveryError::Enumerate)
        .map_err(AccessError::Discovery)?
        .into_iter()
        .find(|process| {
            process.pid == expected.pid()
                && matched_executable_name(process) == Some(target.executable())
        })
        .ok_or(AccessError::TargetNotFound(expected.pid()))?;

    let process = os
        .into_process_by_info(process_info)
        .map_err(|source| AccessError::Open {
            pid: expected.pid(),
            source,
        })?;
    attach_process(target, process)
}

fn native_os() -> Result<NativeOs, DiscoveryError> {
    NativeOs::new(&OsArgs::default()).map_err(DiscoveryError::Initialize)
}

fn attach_process(
    target: &Target,
    mut process: NativeProcess,
) -> Result<AttachedTarget, AccessError> {
    let instance = target.instance();
    let modules = process
        .module_list()
        .map_err(|source| AccessError::Modules {
            pid: instance.pid(),
            source,
        })?;
    let executable = modules
        .iter()
        .find(|module| {
            module
                .name
                .as_ref()
                .eq_ignore_ascii_case(target.executable())
        })
        .ok_or(AccessError::NoExecutableModule(instance.pid()))?;

    let mut probe = [0_u8; 1];
    process
        .read_raw_into(executable.base, &mut probe)
        .data()
        .map_err(|source| AccessError::Read {
            pid: instance.pid(),
            source,
        })?;

    ensure_current(instance)?;

    let proof = AccessProof::new(
        executable.name.as_ref().to_owned(),
        executable.base.to_umem(),
    );
    Ok(AttachedTarget::new(target.clone(), proof, process))
}

fn target_from_process(process: &ProcessInfo) -> Option<Target> {
    let executable = matched_executable_name(process)?;
    let instance = ProcessInstance::for_pid(process.pid)?;
    Some(Target::new(instance, executable.to_owned()))
}

fn matched_executable_name(process: &ProcessInfo) -> Option<&'static str> {
    matched_target_executable(process.name.as_ref())
        .or_else(|| matched_target_executable(process.path.as_ref()))
        .or_else(|| {
            process
                .command_line
                .as_ref()
                .split_whitespace()
                .next()
                .and_then(matched_target_executable)
        })
}

fn matched_target_executable(value: &str) -> Option<&'static str> {
    let basename = executable_basename(value);
    let matched = TARGET_EXECUTABLES
        .iter()
        .copied()
        .find(|candidate| basename.eq_ignore_ascii_case(candidate));
    if matched.is_some() {
        return matched;
    }

    #[cfg(target_os = "linux")]
    if TRUNCATED_TARGET_EXECUTABLES
        .iter()
        .any(|candidate| basename.eq_ignore_ascii_case(candidate))
    {
        return Some("Warframe.x64.exe");
    }

    None
}

fn executable_basename(value: &str) -> &str {
    value
        .trim_matches(['"', '\''])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim_matches(['"', '\''])
}

fn ensure_current(instance: ProcessInstance) -> Result<(), AccessError> {
    if instance.is_current() {
        Ok(())
    } else {
        Err(AccessError::TargetChanged(instance.pid()))
    }
}
