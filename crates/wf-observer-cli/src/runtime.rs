//! Per-user metadata for the active target-bound agent.

use std::{
    fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use memory_reader::{ProcessInstance, Target};

use crate::{paths, prelude::*, singleton::AgentLock};

/// A runtime record owned by one exact agent process instance.
#[derive(Debug, ..Eq, ..Serde)]
struct RuntimeRecord {
    application_version: String,
    agent: RecordedProcess,
    target: RecordedProcess,
    target_executable: String,
    endpoint_id: String,
}

impl RuntimeRecord {
    fn new(target: &Target, endpoint_id: String) -> anyhow::Result<Self> {
        let agent = ProcessInstance::for_pid(std::process::id())
            .context("failed to identify the background agent process")?;

        Ok(Self {
            application_version: env!("CARGO_PKG_VERSION").to_owned(),
            agent: agent.into(),
            target: target.instance().into(),
            target_executable: target.executable().to_owned(),
            endpoint_id,
        })
    }

    fn processes_are_current(&self) -> bool {
        let agent_is_current = self.agent.is_current();
        let target_is_current = self.target.is_current();
        agent_is_current && target_is_current
    }
}

#[derive(Debug, ..Copy, ..Eq, ..Serde)]
struct RecordedProcess {
    pid: u32,
    start_marker: u64,
}

impl RecordedProcess {
    fn is_current(self) -> bool {
        ProcessInstance::for_pid(self.pid)
            .is_some_and(|instance| instance.start_marker() == self.start_marker)
    }
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
    agent: RecordedProcess,
    registered: bool,
}

impl Registration {
    /// Atomically publishes the active agent after its target and transport are ready.
    pub(crate) fn publish(target: &Target, endpoint_id: String) -> anyhow::Result<Self> {
        let record = RuntimeRecord::new(target, endpoint_id)?;
        let path = paths::runtime_record_path()?;
        write_atomic(&path, &record)?;

        Ok(Self {
            path,
            agent: record.agent,
            registered: true,
        })
    }

    /// Removes this agent's record without removing a replacement record.
    pub(crate) fn unregister(mut self) -> anyhow::Result<()> {
        let result = remove_if_owned_by(&self.path, self.agent);
        if result.is_ok() {
            self.registered = false;
        }
        result
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        if self.registered
            && let Err(error) = remove_if_owned_by(&self.path, self.agent)
        {
            warn!(%error, "failed to remove the agent runtime record");
        }
    }
}

/// Prints the current target-bound agent metadata.
pub(crate) fn print_status() -> anyhow::Result<()> {
    let Some(record) = current_record()? else {
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

fn current_record() -> anyhow::Result<Option<RuntimeRecord>> {
    let path = paths::runtime_record_path()?;
    let lock_path = paths::agent_lock_path()?;
    current_record_at(&path, &lock_path)
}

fn current_record_at(path: &Path, lock_path: &Path) -> anyhow::Result<Option<RuntimeRecord>> {
    let Some(bytes) = read_optional(path)? else {
        return Ok(None);
    };

    let record = serde_json::from_slice::<RuntimeRecord>(&bytes).ok();
    if record
        .as_ref()
        .is_some_and(RuntimeRecord::processes_are_current)
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
    let Ok(record) = serde_json::from_slice::<RuntimeRecord>(&bytes) else {
        return Ok(());
    };
    if record.agent == agent {
        remove_optional(path)?;
    }
    Ok(())
}

fn write_atomic(path: &Path, record: &RuntimeRecord) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .context("the runtime record path does not have a parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;

    let mut temporary = tempfile::Builder::new()
        .prefix(".runtime-")
        .tempfile_in(parent)
        .with_context(|| format!("failed to create a runtime record in {}", parent.display()))?;
    serde_json::to_writer(&mut temporary, record).context("failed to encode the runtime record")?;
    temporary
        .write_all(b"\n")
        .context("failed to finish the runtime record")?;
    temporary
        .flush()
        .context("failed to flush the runtime record")?;
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

    fn current_process() -> anyhow::Result<RecordedProcess> {
        ProcessInstance::for_pid(std::process::id())
            .map(Into::into)
            .context("failed to identify the test process")
    }

    fn record(agent: RecordedProcess, target: RecordedProcess) -> RuntimeRecord {
        RuntimeRecord {
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

        assert_eq!(current_record_at(&path, &lock_path)?, Some(expected));
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

        assert_eq!(current_record_at(&path, &lock_path)?, None);
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

        assert_eq!(current_record_at(&path, &lock_path)?, None);
        assert!(path.exists());
        drop(lock);
        Ok(())
    }

    #[test]
    fn unregister_does_not_remove_a_replacement_record() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("runtime.json");
        let owner = RecordedProcess {
            pid: 10,
            start_marker: 20,
        };
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
        assert_eq!(
            serde_json::from_slice::<RuntimeRecord>(&bytes)?,
            replacement
        );
        Ok(())
    }

    #[test]
    fn unregister_removes_its_own_record() -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("runtime.json");
        let owner = current_process()?;
        write_atomic(&path, &record(owner, owner))?;

        Registration {
            path: path.clone(),
            agent: owner,
            registered: true,
        }
        .unregister()?;

        assert!(!path.exists());
        Ok(())
    }
}
