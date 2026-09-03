#[macro_use(derive)]
extern crate derive_aliases;

mod agent;
mod application;
mod cli;
mod derive_alias;
mod identity;
mod launch;
mod paths;
mod prelude;
mod runtime;
mod singleton;
mod startup;
mod transport;

use std::process::ExitCode;

use clap::Parser;

use crate::prelude::*;

#[tokio::main]
#[hotpath::main]
async fn main() -> ExitCode {
    hotpath::tokio_runtime!();

    let args = cli::Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(args.verbosity)
        .with_writer(std::io::stderr)
        .init();

    let result = match args.command() {
        cli::Command::Attach => launch::attach().await,
        cli::Command::Status => runtime::print_status(),
        cli::Command::Stop => runtime::stop().await,
        cli::Command::Serve {
            print_ticket,
            shutdown_on_stdin_close,
        } => application::run(print_ticket, shutdown_on_stdin_close).await,
        cli::Command::Agent { pid, start_marker } => agent::run(pid, start_marker).await,
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
