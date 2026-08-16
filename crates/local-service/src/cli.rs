use clap::{Parser, Subcommand};

/// Runs the local Warframe Observer application.
#[derive(Debug, Parser)]
#[command(version, arg_required_else_help = true)]
pub(crate) struct Cli {
    #[command(flatten)]
    pub verbosity: clap_verbosity_flag::Verbosity,
    #[command(subcommand)]
    command: Command,
}

impl Cli {
    /// Returns the requested command.
    pub(crate) fn command(self) -> Command {
        self.command
    }
}

/// Top-level application commands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Runs in the foreground.
    Run,
    /// Checks for and applies an available update.
    Update,
}
