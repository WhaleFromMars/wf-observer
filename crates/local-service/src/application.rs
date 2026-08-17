//! Foreground application lifecycle.

use anyhow::Context as _;

use crate::prelude::*;

/// Runs until an operating-system shutdown signal arrives.
pub(crate) async fn run() -> anyhow::Result<()> {
    let server = crate::transport::start().await?;
    info!(endpoint = ?server.endpoint().addr(), "local application started");

    let shutdown_result = wait_for_shutdown().await;
    if shutdown_result.is_ok() {
        info!("shutdown requested");
    }

    let server_result = server
        .shutdown()
        .await
        .context("failed to shut down the local transport");
    info!("local application stopped");

    shutdown_result?;
    server_result
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn ping_round_trip() -> anyhow::Result<()> {
        let server = crate::transport::start().await?;
        let address = server.endpoint().addr();

        let exchange_result = async {
            let client = wf_observer::Client::connect(address).await?;
            let ping_result = client.ping().await;
            client.close().await;
            ping_result
        }
        .await;

        let shutdown_result = server
            .shutdown()
            .await
            .context("failed to shut down the test Iroh transport");

        exchange_result?;
        shutdown_result
    }
}
