//! Public attachment command and background-agent launch.

use std::{path::PathBuf, process::Stdio, time::Duration};

use anyhow::{Context as _, bail};
use memory_reader::Target;
use tokio::process::{Child, Command};

use crate::startup::{self, Status};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const FAILED_CHILD_EXIT_TIMEOUT: Duration = Duration::from_secs(2);

/// Discovers Warframe, proves access, and starts its background agent.
pub(crate) async fn attach() -> anyhow::Result<()> {
    let target = discover_target()?;
    let instance = target.instance();

    println!("Found {} (PID {})", target.executable(), instance.pid());

    let attachment = memory_reader::attach(&target).with_context(|| {
        format!(
            "failed to access {} process {}",
            target.executable(),
            instance.pid()
        )
    })?;
    drop(attachment);

    let mut child = spawn_agent(instance.pid(), instance.start_marker())?;
    let agent_pid = child
        .id()
        .context("the operating system did not report the background agent PID")?;
    let stdout = child
        .stdout
        .take()
        .context("the background agent startup channel was not captured")?;

    let status = match tokio::time::timeout(STARTUP_TIMEOUT, startup::read(stdout)).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            reap_failed_child(&mut child).await?;
            return Err(error).context("failed to read the background agent startup status");
        }
        Err(_) => {
            terminate_child(&mut child).await?;
            bail!("background agent did not start within {STARTUP_TIMEOUT:?}");
        }
    };

    match status {
        Status::Ready => {
            if let Some(exit_status) = child
                .try_wait()
                .context("failed to inspect the background agent")?
            {
                bail!("background agent exited during startup: {exit_status}");
            }

            println!("Agent started (PID {agent_pid})");
            Ok(())
        }
        Status::Failed(message) => {
            reap_failed_child(&mut child).await?;
            bail!("{message}");
        }
    }
}

fn discover_target() -> anyhow::Result<Target> {
    let mut targets =
        memory_reader::discover_targets().context("failed to discover Warframe processes")?;

    match targets.len() {
        0 => bail!("Warframe is not running"),
        1 => targets
            .pop()
            .context("target discovery returned an inconsistent result"),
        _ => {
            let pids = targets
                .iter()
                .map(|target| target.instance().pid().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            bail!("multiple Warframe processes are running (PIDs: {pids})");
        }
    }
}

fn spawn_agent(pid: u32, start_marker: u64) -> anyhow::Result<Child> {
    let executable = current_executable()?;
    let mut command = Command::new(executable);
    command
        .arg("_agent")
        .arg("--pid")
        .arg(pid.to_string())
        .arg("--start-marker")
        .arg(start_marker.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_detached(&mut command);

    command
        .spawn()
        .context("failed to start the background agent")
}

fn current_executable() -> anyhow::Result<PathBuf> {
    std::env::current_exe().context("failed to locate the current wf-observer executable")
}

async fn reap_failed_child(child: &mut Child) -> anyhow::Result<()> {
    match tokio::time::timeout(FAILED_CHILD_EXIT_TIMEOUT, child.wait()).await {
        Ok(result) => {
            result.context("failed to wait for the failed background agent")?;
            Ok(())
        }
        Err(_) => terminate_child(child).await,
    }
}

async fn terminate_child(child: &mut Child) -> anyhow::Result<()> {
    if child
        .try_wait()
        .context("failed to inspect the background agent before termination")?
        .is_some()
    {
        return Ok(());
    }

    child
        .start_kill()
        .context("failed to terminate the background agent")?;
    child
        .wait()
        .await
        .context("failed to reap the background agent")?;
    Ok(())
}

#[cfg(windows)]
fn configure_detached(command: &mut Command) {
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};

    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(windows))]
fn configure_detached(_command: &mut Command) {}
