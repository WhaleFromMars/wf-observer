//! Public attachment command and background-agent launch.

use std::{path::PathBuf, process::Stdio, time::Duration};

use anyhow::{Context as _, bail};
use memory_reader::Target;
use tokio::process::{Child, Command};

use crate::{
    paths,
    runtime::{self, AgentInfo},
    singleton::AgentLock,
    startup::{self, Status},
};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const FAILED_CHILD_EXIT_TIMEOUT: Duration = Duration::from_secs(2);
const RECONCILIATION_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Discovers Warframe, proves access, and starts its background agent.
pub(crate) async fn attach() -> anyhow::Result<()> {
    let target = discover_target()?;
    let instance = target.instance();

    println!("Found {} (PID {})", target.executable(), instance.pid());

    if reconcile_existing_agent(&target).await? {
        return Ok(());
    }

    let attachment = memory_reader::attach(&target).with_context(|| {
        format!(
            "failed to access {} process {}",
            target.executable(),
            instance.pid()
        )
    })?;
    drop(attachment);

    launch_agent(&target).await
}

async fn launch_agent(target: &Target) -> anyhow::Result<()> {
    let mut retries_remaining = 1;

    loop {
        match launch_agent_once(target).await {
            Ok(agent_pid) => {
                println!("Agent started (PID {agent_pid})");
                return Ok(());
            }
            Err(error) => match competing_agent(target).await {
                Ok(CompetingAgent::Compatible) => {
                    report_already_attached(target);
                    return Ok(());
                }
                Ok(CompetingAgent::Vacant) if retries_remaining > 0 => {
                    retries_remaining -= 1;
                }
                Ok(CompetingAgent::Incompatible | CompetingAgent::Vacant) => return Err(error),
                Err(reconciliation_error) => {
                    return Err(error.context(format!(
                        "failed to inspect the competing background agent: \
                         {reconciliation_error}"
                    )));
                }
            },
        }
    }
}

async fn launch_agent_once(target: &Target) -> anyhow::Result<u32> {
    let instance = target.instance();
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
            return Err(anyhow::Error::new(error)
                .context("failed to read the background agent startup status"));
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

            Ok(agent_pid)
        }
        Status::Failed(message) => {
            reap_failed_child(&mut child).await?;
            Err(anyhow::Error::msg(message))
        }
    }
}

async fn reconcile_existing_agent(target: &Target) -> anyhow::Result<bool> {
    let Some(agent) = runtime::current_agent()? else {
        return Ok(false);
    };

    if is_compatible(&agent, target) {
        report_already_attached(target);
        return Ok(true);
    }

    if agent.version() != env!("CARGO_PKG_VERSION") {
        println!("Existing agent version: {}", agent.version());
        println!("Installed version:      {}", env!("CARGO_PKG_VERSION"));
    }
    if !agent.is_attached_to(target.instance()) {
        println!("Existing agent is attached to an old Warframe process.");
    }

    println!("Replacing existing agent...");
    runtime::stop_agent(&agent)
        .await
        .context("failed to stop the existing background agent")?;
    Ok(false)
}

fn is_compatible(agent: &AgentInfo, target: &Target) -> bool {
    agent.is_compatible_with(env!("CARGO_PKG_VERSION"), target.instance())
}

fn report_already_attached(target: &Target) {
    println!("Agent already attached to PID {}", target.instance().pid());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompetingAgent {
    Compatible,
    Incompatible,
    Vacant,
}

async fn competing_agent(target: &Target) -> anyhow::Result<CompetingAgent> {
    let lock_path = paths::agent_lock_path()?;
    let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;

    loop {
        if let Some(agent) = runtime::current_agent()? {
            return Ok(if is_compatible(&agent, target) {
                CompetingAgent::Compatible
            } else {
                CompetingAgent::Incompatible
            });
        }

        if let Some(lock) = AgentLock::try_acquire(&lock_path)? {
            drop(lock);
            return Ok(CompetingAgent::Vacant);
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("competing background agent did not become ready within {STARTUP_TIMEOUT:?}");
        }

        tokio::time::sleep(RECONCILIATION_POLL_INTERVAL).await;
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
