use std::{path::PathBuf, process::ExitCode};

use anyhow::Context as _;
use local_service::{instance::InstanceGuard, update::UpdatePaths};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let mut arguments = std::env::args_os().skip(1).map(PathBuf::from);
    let state = arguments.next().context("missing state directory")?;
    let ready = arguments.next().context("missing ready-file path")?;
    anyhow::ensure!(arguments.next().is_none(), "unexpected extra argument");

    let paths = UpdatePaths::under(&state);
    let instance = InstanceGuard::acquire(paths.coordination())?;
    anyhow::ensure!(
        !instance.update_requested()?,
        "update request was already active"
    );
    std::fs::write(&ready, b"ready")
        .with_context(|| format!("failed to write {}", ready.display()))?;
    instance.wait_for_update_request().await
}
