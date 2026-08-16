//! Foreground application lifecycle.

use anyhow::Context as _;

use crate::prelude::*;

/// Runs until an operating-system shutdown signal arrives.
pub(crate) async fn run() -> anyhow::Result<()> {
    info!("local application started");

    wait_for_shutdown().await?;

    info!("shutdown requested");
    info!("local application stopped");

    Ok(())
}

#[cfg(not(unix))]
async fn wait_for_shutdown() -> anyhow::Result<()> {
    tokio::signal::ctrl_c()
        .await
        .context("failed to listen for Ctrl+C")
}

#[cfg(unix)]
async fn wait_for_shutdown() -> anyhow::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate()).context("failed to listen for SIGTERM")?;

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result.context("failed to listen for SIGINT")?;
        }
        signal = terminate.recv() => {
            signal.context("SIGTERM listener ended unexpectedly")?;
        }
    }

    Ok(())
}
