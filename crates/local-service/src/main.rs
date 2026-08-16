#[macro_use(derive)]
extern crate derive_aliases;

mod application;
mod cli;
mod derive_alias;
mod prelude;
mod update;

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
        .init();

    let result = match args.command() {
        cli::Command::Run => application::run().await,
        cli::Command::Update => Ok(()),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
