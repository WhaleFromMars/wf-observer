//! Foreground application lifecycle.

use std::io::{self, Write as _};

use anyhow::Context as _;
use iroh_tickets::endpoint::EndpointTicket;

use crate::prelude::*;

/// Runs until an operating-system shutdown signal arrives.
pub(crate) async fn run(print_ticket: bool) -> anyhow::Result<()> {
    let secret_key = crate::identity::load_or_create()?;
    let server = crate::transport::start(secret_key).await?;

    if print_ticket {
        if tokio::time::timeout(
            std::time::Duration::from_secs(iroh::NET_REPORT_TIMEOUT),
            server.endpoint().online(),
        )
        .await
        .is_err()
        {
            warn!("relay registration timed out; printing the available endpoint addresses");
        }

        let ticket = EndpointTicket::new(server.endpoint().addr());
        let mut stdout = io::stdout().lock();

        writeln!(stdout, "WF_OBSERVER_ENDPOINT_TICKET={ticket}")
            .context("failed to print the endpoint ticket")?;
        stdout
            .flush()
            .context("failed to flush the endpoint ticket")?;
    }

    info!(endpoint_id = %server.endpoint().id(), "local application started");

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

/// Prints the endpoint identifier shared with clients.
pub(crate) fn print_endpoint() -> anyhow::Result<()> {
    println!("{}", crate::identity::load_or_create()?.public());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn ping_round_trip() -> anyhow::Result<()> {
        let server = crate::transport::start(iroh::SecretKey::generate()).await?;
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

    #[tokio::test(flavor = "multi_thread")]
    async fn ffi_ping_round_trip() -> anyhow::Result<()> {
        let server = crate::transport::start(iroh::SecretKey::generate()).await?;
        let ticket = iroh_tickets::endpoint::EndpointTicket::new(server.endpoint().addr());

        let exchange_result = async {
            let client = wf_observer_ffi::connect(ticket.to_string())
                .await
                .map_err(anyhow::Error::msg)?;
            client.ping().await.map_err(anyhow::Error::msg)?;
            client.shutdown().await.map_err(anyhow::Error::msg)
        }
        .await;

        let shutdown_result = server
            .shutdown()
            .await
            .context("failed to shut down the FFI test Iroh transport");

        exchange_result?;
        shutdown_result
    }
}
