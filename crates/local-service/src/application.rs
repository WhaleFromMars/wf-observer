//! Foreground application lifecycle.

use std::io::{self, Write as _};

use anyhow::Context as _;
use iroh_tickets::endpoint::EndpointTicket;
use local_service::{instance::InstanceGuard, update::UpdatePaths};

use crate::prelude::*;

/// Runs until an operating-system or supervising-process shutdown arrives.
pub(crate) async fn run(print_ticket: bool, shutdown_on_stdin_close: bool) -> anyhow::Result<()> {
    let update_paths = UpdatePaths::discover()?;
    let instance = InstanceGuard::acquire(update_paths.coordination())?;
    anyhow::ensure!(
        !instance.update_requested()?,
        "an update is ready to install; wait for it to finish before starting"
    );

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

    let shutdown_result = wait_for_shutdown(shutdown_on_stdin_close, &instance).await;
    if let Ok(reason) = &shutdown_result {
        match reason {
            ShutdownReason::OperatingSystem => info!("operating-system shutdown requested"),
            ShutdownReason::StdinClosed => info!("supervising process closed standard input"),
            ShutdownReason::Update => info!("update shutdown requested"),
        }
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

#[derive(Clone, Copy, Debug)]
enum ShutdownReason {
    OperatingSystem,
    StdinClosed,
    Update,
}

async fn wait_for_shutdown(
    shutdown_on_stdin_close: bool,
    instance: &InstanceGuard,
) -> anyhow::Result<ShutdownReason> {
    tokio::select! {
        result = wait_for_operating_system_shutdown() => {
            result.map(|()| ShutdownReason::OperatingSystem)
        },
        result = wait_for_stdin_close(), if shutdown_on_stdin_close => {
            result.map(|()| ShutdownReason::StdinClosed)
        },
        result = instance.wait_for_update_request() => result.map(|()| ShutdownReason::Update),
    }
}

async fn wait_for_stdin_close() -> anyhow::Result<()> {
    use tokio::io::AsyncReadExt as _;

    let mut stdin = tokio::io::stdin();
    let mut buffer = [0_u8; 1024];

    loop {
        if stdin
            .read(&mut buffer)
            .await
            .context("failed to listen for the supervising process")?
            == 0
        {
            return Ok(());
        }
    }
}

#[cfg(not(unix))]
async fn wait_for_operating_system_shutdown() -> anyhow::Result<()> {
    tokio::signal::ctrl_c()
        .await
        .context("failed to listen for Ctrl+C")
}

#[cfg(unix)]
async fn wait_for_operating_system_shutdown() -> anyhow::Result<()> {
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
