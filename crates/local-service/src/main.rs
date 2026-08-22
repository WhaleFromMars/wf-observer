#[macro_use(derive)]
extern crate derive_aliases;

mod application;
mod cli;
mod derive_alias;
mod identity;
mod prelude;
mod transport;

use std::process::ExitCode;

use clap::Parser;
use local_service::update::{UpdateConfig, UpdateOutcome};

use crate::prelude::*;

#[tokio::main]
#[hotpath::main]
async fn main() -> ExitCode {
    hotpath::tokio_runtime!();

    let args = cli::Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(args.verbosity)
        .init();

    let result = match args.command() {
        cli::Command::Run {
            print_ticket,
            shutdown_on_stdin_close,
        } => application::run(print_ticket, shutdown_on_stdin_close).await,
        cli::Command::Endpoint => application::print_endpoint(),
        cli::Command::Update => update().await,
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn update() -> anyhow::Result<()> {
    match local_service::update::execute(UpdateConfig::production()?).await? {
        UpdateOutcome::Current(version) => println!("already up to date ({version})"),
        UpdateOutcome::Installed(version) => {
            println!("updated to {version}; start the foreground process normally");
        }
    }
    Ok(())
}
