//! Per-user filesystem locations used by Warframe Observer.

use std::path::PathBuf;

use anyhow::Context as _;
use directories::ProjectDirs;

const APPLICATION_NAME: &str = "wf-observer";
const AGENT_LOCK_FILE: &str = "agent.lock";
const RUNTIME_RECORD_FILE: &str = "runtime.json";
const SHUTDOWN_REQUEST_FILE: &str = "shutdown.json";

/// Resolves the operating system's application directories for the current user.
pub(crate) fn project_directories() -> anyhow::Result<ProjectDirs> {
    ProjectDirs::from("", "", APPLICATION_NAME)
        .context("the operating system did not provide per-user application directories")
}

/// Returns the transient singleton-lock path for the current user.
pub(crate) fn agent_lock_path() -> anyhow::Result<PathBuf> {
    runtime_file(AGENT_LOCK_FILE)
}

/// Returns the transient runtime-record path for the current user.
pub(crate) fn runtime_record_path() -> anyhow::Result<PathBuf> {
    runtime_file(RUNTIME_RECORD_FILE)
}

/// Returns the transient shutdown-request path for the current user.
pub(crate) fn shutdown_request_path() -> anyhow::Result<PathBuf> {
    runtime_file(SHUTDOWN_REQUEST_FILE)
}

fn runtime_file(name: &str) -> anyhow::Result<PathBuf> {
    let project = project_directories()?;
    let directory = project.runtime_dir().unwrap_or_else(|| project.cache_dir());

    Ok(directory.join(name))
}
