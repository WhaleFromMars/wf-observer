//! Per-user metadata for the active target-bound agent.

use std::{
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, bail};
use memory_reader::{ProcessInstance, Target};

use crate::{paths, prelude::*, singleton::AgentLock};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// A runtime record owned by one exact agent process instance.
#[derive(Debug, ..Eq, ..Serde)]
pub(crate) struct AgentInfo {
    application_version: String,
    agent: RecordedProcess,
    target: RecordedProcess,
    target_executable: String,
    endpoint_id: String,
}

impl AgentInfo {
    fn new(agent: ProcessInstance, target: &Target, endpoint_id: String) -> Self {
        Self {
            application_version: env!("CARGO_PKG_VERSION").to_owned(),
            agent: agent.into(),
            target: target.instance().into(),
            target_executable: target.executable().to_owned(),
            endpoint_id,
        }
    }

    fn processes_are_current(&self) -> bool {
        let agent_is_current = self.agent.is_current();
        let target_is_current = self.target.is_current();
        agent_is_current && target_is_current
    }

    /// Returns the version of the executable which launched this agent.
    pub(crate) fn version(&self) -> &str {
        &self.application_version
    }

    /// Returns the agent's operating-system process identifier.
    pub(crate) const fn pid(&self) -> u32 {
        self.agent.pid
    }

    /// Returns whether the agent is attached to `target`.
    pub(crate) fn is_attached_to(&self, target: ProcessInstance) -> bool {
        self.target == target.into()
    }

    /// Returns whether this agent already satisfies an attachment request.
    pub(crate) fn is_compatible_with(&self, version: &str, target: ProcessInstance) -> bool {
        self.version() == version && self.is_attached_to(target)
    }
}

#[derive(Debug, ..Copy, ..Eq, ..Serde)]
struct RecordedProcess {
    pid: u32,
    start_marker: u64,
}

impl RecordedProcess {
    fn current_instance(self) -> Option<ProcessInstance> {
        ProcessInstance::for_pid(self.pid)
            .filter(|instance| instance.start_marker() == self.start_marker)
    }

    fn is_current(self) -> bool {
        self.current_instance().is_some()
    }
}

#[derive(Debug, ..Eq, ..Serde)]
struct ShutdownRequest {
    agent: RecordedProcess,
}

impl From<ProcessInstance> for RecordedProcess {
    fn from(instance: ProcessInstance) -> Self {
        Self {
            pid: instance.pid(),
            start_marker: instance.start_marker(),
        }
    }
}

/// Removes a published record when its owning agent shuts down.
pub(crate) struct Registration {
    path: PathBuf,
    agent: ProcessInstance,
    registered: bool,
}

impl Registration {
    /// Atomically publishes the active agent after its target and transport are ready.
    pub(crate) fn publish(target: &Target, endpoint_id: String) -> anyhow::Result<Self> {
        let agent = ProcessInstance::for_pid(std::process::id())
            .context("failed to identify the background agent process")?;
        let record = AgentInfo::new(agent, target, endpoint_id);
        let path = paths::runtime_record_path()?;
        remove_optional(&paths::shutdown_request_path()?)
            .context("failed to clear the stale shutdown request")?;
        write_atomic(&path, &record)?;

        Ok(Self {
            path,
            agent,
            registered: true,
        })
    }

    /// Removes this agent's record without removing a replacement record.
    pub(crate) fn unregister(mut self) -> anyhow::Result<()> {
        let result = remove_if_owned_by(&self.path, self.agent.into());
        if result.is_ok() {
            self.registered = false;
        }
        result
    }

    /// Returns the exact process instance which owns this registration.
    pub(crate) const fn agent(&self) -> ProcessInstance {
        self.agent
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        if self.registered
            && let Err(error) = remove_if_owned_by(&self.path, self.agent.into())
        {
            warn!(%error, "failed to remove the agent runtime record");
        }
    }
}

/// Prints the current target-bound agent metadata.
pub(crate) fn print_status() -> anyhow::Result<()> {
    let Some(record) = current_agent()? else {
        println!("Agent: not running");
        return Ok(());
    };

    println!("Agent: running");
    println!("Version: {}", record.application_version);
    println!("Agent PID: {}", record.agent.pid);
    println!("Target: {}", record.target_executable);
    println!("Target PID: {}", record.target.pid);
    println!("Iroh endpoint ID: {}", record.endpoint_id);
    Ok(())
}

/// Requests cooperative shutdown and waits for the current agent to exit.
pub(crate) async fn stop() -> anyhow::Result<()> {
    let Some(record) = current_agent()? else {
        println!("Agent is not running");
        return Ok(());
    };

    println!("Stopping agent (PID {})...", record.pid());
    stop_agent(&record).await?;
    println!("Agent stopped");
    Ok(())
}

/// Requests cooperative shutdown of `agent` and waits for that process instance to exit.
pub(crate) async fn stop_agent(agent: &AgentInfo) -> anyhow::Result<()> {
    let instance = agent.agent;
    let path = paths::shutdown_request_path()?;
    write_atomic(&path, &ShutdownRequest { agent: instance })?;

    wait_for_exit(instance).await?;
    remove_shutdown_request_at(&path, instance)?;
    Ok(())
}

/// Returns whether the current shutdown request names `agent`.
pub(crate) fn shutdown_requested(agent: ProcessInstance) -> anyhow::Result<bool> {
    shutdown_requested_at(&paths::shutdown_request_path()?, agent.into())
}

/// Removes a shutdown request only when it names `agent`.
pub(crate) fn clear_shutdown_request(agent: ProcessInstance) -> anyhow::Result<()> {
    remove_shutdown_request_at(&paths::shutdown_request_path()?, agent.into())
}

/// Returns the active agent after independently validating it and its target.
pub(crate) fn current_agent() -> anyhow::Result<Option<AgentInfo>> {
    let path = paths::runtime_record_path()?;
    let lock_path = paths::agent_lock_path()?;
    current_agent_at(&path, &lock_path)
}

fn current_agent_at(path: &Path, lock_path: &Path) -> anyhow::Result<Option<AgentInfo>> {
    let Some(bytes) = read_optional(path)? else {
        return Ok(None);
    };

    let record = serde_json::from_slice::<AgentInfo>(&bytes).ok();
    if record
        .as_ref()
        .is_some_and(AgentInfo::processes_are_current)
    {
        return Ok(record);
    }

    remove_stale_record(path, lock_path, &bytes)?;
    Ok(None)
}

fn remove_stale_record(path: &Path, lock_path: &Path, expected: &[u8]) -> anyhow::Result<()> {
    let Some(lock) = AgentLock::try_acquire(lock_path)? else {
        return Ok(());
    };

    if read_optional(path)?.as_deref() == Some(expected) {
        remove_optional(path)?;
    }
    drop(lock);
    Ok(())
}

fn remove_if_owned_by(path: &Path, agent: RecordedProcess) -> anyhow::Result<()> {
    let Some(bytes) = read_optional(path)? else {
        return Ok(());
    };
    let Ok(record) = serde_json::from_slice::<AgentInfo>(&bytes) else {
        return Ok(());
    };
    if record.agent == agent {
        remove_optional(path)?;
    }
    Ok(())
}

async fn wait_for_exit(agent: RecordedProcess) -> anyhow::Result<()> {
    let result = tokio::time::timeout(SHUTDOWN_TIMEOUT, async {
        while agent.is_current() {
            tokio::time::sleep(SHUTDOWN_POLL_INTERVAL).await;
        }
    })
    .await;

    if result.is_err() {
        bail!(
            "agent process {} did not stop within {SHUTDOWN_TIMEOUT:?}",
            agent.pid
        );
    }
    Ok(())
}

fn shutdown_requested_at(path: &Path, agent: RecordedProcess) -> anyhow::Result<bool> {
    let Some(bytes) = read_optional(path)? else {
        return Ok(false);
    };
    let Ok(request) = serde_json::from_slice::<ShutdownRequest>(&bytes) else {
        return Ok(false);
    };
    Ok(request.agent == agent)
}

fn remove_shutdown_request_at(path: &Path, agent: RecordedProcess) -> anyhow::Result<()> {
    let Some(bytes) = read_optional(path)? else {
        return Ok(());
    };
    let Ok(request) = serde_json::from_slice::<ShutdownRequest>(&bytes) else {
        return Ok(());
    };
    if request.agent == agent {
        remove_optional(path)?;
    }
    Ok(())
}

fn write_atomic(path: &Path, value: &impl serde::Serialize) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .context("the runtime state path does not have a parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let mut temporary = tempfile::Builder::new()
        .prefix(".state-")
        .tempfile_in(parent)
        .with_context(|| format!("failed to create runtime state in {}", parent.display()))?;
    serde_json::to_writer(&mut temporary, value).context("failed to encode the runtime state")?;
    temporary
        .write_all(b"\n")
        .context("failed to finish the runtime state")?;
    temporary
        .flush()
        .context("failed to flush the runtime state")?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to persist {}", path.display()))?;
    Ok(())
}

fn read_optional(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn remove_optional(path: &Path) -> anyhow::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_instance() -> anyhow::Result<ProcessInstance> {
        ProcessInstance::for_pid(std::process::id()).context("failed to identify the test process")
    }

    fn current_process() -> anyhow::Result<RecordedProcess> {
        current_instance().map(Into::into)
    }

    fn record(agent: RecordedProcess, target: RecordedProcess) -> AgentInfo {
        AgentInfo {
            application_version: "1.2.3".to_owned(),
            agent,
            target,
            target_executable: "Warframe.x64.exe".to_owned(),
            endpoint_id: "test-endpoint".to_owned(),
        }
    }

    #[test]
    fn reads_a_record_while_both_processes_are_current() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("runtime.json");
        let lock_path = directory.path().join("agent.lock");
        let process = current_process()?;
        let expected = record(process, process);
        write_atomic(&path, &expected)?;

        assert_eq!(current_agent_at(&path, &lock_path)?, Some(expected));
        assert!(path.exists());
        Ok(())
    }

    #[test]
    fn removes_a_stale_record_while_no_agent_owns_the_lock() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("runtime.json");
        let lock_path = directory.path().join("agent.lock");
        let stale = RecordedProcess {
            pid: u32::MAX,
            start_marker: u64::MAX,
        };
        write_atomic(&path, &record(stale, current_process()?))?;

        assert_eq!(current_agent_at(&path, &lock_path)?, None);
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn leaves_a_stale_record_while_an_agent_owns_the_lock() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("runtime.json");
        let lock_path = directory.path().join("agent.lock");
        let stale = RecordedProcess {
            pid: u32::MAX,
            start_marker: u64::MAX,
        };
        write_atomic(&path, &record(stale, current_process()?))?;
        let lock = AgentLock::acquire(&lock_path)?;

        assert_eq!(current_agent_at(&path, &lock_path)?, None);
        assert!(path.exists());
        drop(lock);
        Ok(())
    }

    #[test]
    fn unregister_does_not_remove_a_replacement_record() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("runtime.json");
        let owner = current_instance()?;
        let replacement = record(
            RecordedProcess {
                pid: 30,
                start_marker: 40,
            },
            current_process()?,
        );
        write_atomic(&path, &replacement)?;

        Registration {
            path: path.clone(),
            agent: owner,
            registered: true,
        }
        .unregister()?;

        let bytes = fs::read(path)?;
        assert_eq!(serde_json::from_slice::<AgentInfo>(&bytes)?, replacement);
        Ok(())
    }

    #[test]
    fn unregister_removes_its_own_record() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("runtime.json");
        let owner = current_instance()?;
        write_atomic(&path, &record(owner.into(), owner.into()))?;

        Registration {
            path: path.clone(),
            agent: owner,
            registered: true,
        }
        .unregister()?;

        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn shutdown_request_applies_only_to_its_agent() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("shutdown.json");
        let requested = current_process()?;
        let other = RecordedProcess {
            pid: requested.pid.wrapping_add(1),
            start_marker: requested.start_marker,
        };
        write_atomic(&path, &ShutdownRequest { agent: requested })?;

        assert!(shutdown_requested_at(&path, requested)?);
        assert!(!shutdown_requested_at(&path, other)?);

        remove_shutdown_request_at(&path, other)?;
        assert!(path.exists());
        remove_shutdown_request_at(&path, requested)?;
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn compatibility_requires_the_same_version_and_target() -> anyhow::Result<()> {
        let target = current_instance()?;
        let info = record(current_process()?, target.into());

        assert!(info.is_compatible_with("1.2.3", target));
        assert!(!info.is_compatible_with("2.0.0", target));
        assert!(
            !record(
                current_process()?,
                RecordedProcess {
                    pid: target.pid().wrapping_add(1),
                    start_marker: target.start_marker(),
                },
            )
            .is_compatible_with("1.2.3", target)
        );
        Ok(())
    }
}
