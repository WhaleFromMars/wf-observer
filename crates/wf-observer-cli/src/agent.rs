//! Target-bound background agent lifecycle.

use std::time::Duration;

use anyhow::{Context as _, ensure};
use memory_reader::{AttachedTarget, ProcessInstance, Target};

use crate::{
    application::{self, RunningApplication},
    paths,
    prelude::*,
    runtime::Registration,
    singleton::AgentLock,
    startup,
};

const TARGET_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Runs the hidden background-agent entrypoint.
pub(crate) async fn run(pid: u32, start_marker: u64) -> anyhow::Result<()> {
    let agent = match RunningAgent::start(pid, start_marker).await {
        Ok(agent) => agent,
        Err(error) => return Err(report_startup_failure(error)),
    };

    if let Err(error) = startup::report_ready().context("failed to report agent readiness") {
        return agent.shutdown(Err(error)).await;
    }

    agent.run().await
}

struct RunningAgent {
    // Keep this before `application` so fallback cleanup runs while its lock is held.
    registration: Registration,
    application: RunningApplication,
    attachment: AttachedTarget,
}

impl RunningAgent {
    async fn start(pid: u32, start_marker: u64) -> anyhow::Result<Self> {
        detach_current_process()?;

        let lock = AgentLock::acquire(&paths::agent_lock_path()?)?;
        let target = rediscover_target(pid, start_marker)?;
        let attachment = memory_reader::attach(&target)
            .with_context(|| format!("failed to attach to target process {pid}"))?;
        let instance = attachment.target().instance();
        let application = RunningApplication::start_with_lock(lock).await?;
        let registration = match Registration::publish(
            attachment.target(),
            application.endpoint().id().to_string(),
        ) {
            Ok(registration) => registration,
            Err(error) => {
                return match application.shutdown().await {
                    Ok(()) => Err(error),
                    Err(shutdown_error) => Err(error.context(format!(
                        "failed to shut down after the runtime record could not be published: \
                         {shutdown_error}"
                    ))),
                };
            }
        };

        info!(
            target_pid = instance.pid(),
            executable = attachment.target().executable(),
            executable_mapping = attachment.proof().executable_mapping(),
            endpoint_id = %application.endpoint().id(),
            "background agent started"
        );

        Ok(Self {
            registration,
            application,
            attachment,
        })
    }

    async fn run(self) -> anyhow::Result<()> {
        let shutdown = wait_for_shutdown(self.attachment.target().instance()).await;
        self.shutdown(shutdown).await
    }

    async fn shutdown(self, reason: anyhow::Result<ShutdownReason>) -> anyhow::Result<()> {
        if let Ok(reason) = reason.as_ref() {
            match reason {
                ShutdownReason::Requested => info!("agent shutdown requested"),
                ShutdownReason::TargetExited => info!("target process exited"),
            }
        }

        let Self {
            application,
            attachment,
            registration,
        } = self;
        let unregister = registration.unregister();
        let shutdown = application.shutdown().await;
        drop(attachment);
        info!("background agent stopped");

        reason?;
        unregister?;
        shutdown
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShutdownReason {
    Requested,
    TargetExited,
}

fn rediscover_target(pid: u32, start_marker: u64) -> anyhow::Result<Target> {
    let current = ProcessInstance::for_pid(pid)
        .with_context(|| format!("target process {pid} no longer exists"))?;
    ensure!(
        current.start_marker() == start_marker,
        "target process {pid} changed before the agent started"
    );

    memory_reader::discover_targets()
        .context("failed to rediscover the target process")?
        .into_iter()
        .find(|target| target.instance() == current)
        .with_context(|| format!("process {pid} is no longer a supported target"))
}

async fn wait_for_shutdown(instance: ProcessInstance) -> anyhow::Result<ShutdownReason> {
    tokio::select! {
        result = application::wait_for_operating_system_shutdown() => {
            result?;
            Ok(ShutdownReason::Requested)
        }
        () = wait_for_target_exit(instance) => Ok(ShutdownReason::TargetExited),
    }
}

async fn wait_for_target_exit(instance: ProcessInstance) {
    loop {
        tokio::time::sleep(TARGET_POLL_INTERVAL).await;
        if !instance.is_current() {
            return;
        }
    }
}

fn report_startup_failure(error: anyhow::Error) -> anyhow::Error {
    match startup::send_failure(&error) {
        Ok(()) => error,
        Err(report_error) => error.context(format!(
            "failed to report the agent startup failure: {report_error}"
        )),
    }
}

#[cfg(unix)]
fn detach_current_process() -> anyhow::Result<()> {
    rustix::process::setsid()
        .map(|_| ())
        .context("failed to detach the background agent from the invoking terminal")
}

#[cfg(not(unix))]
fn detach_current_process() -> anyhow::Result<()> {
    Ok(())
}
