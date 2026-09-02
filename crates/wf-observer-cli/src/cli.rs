use clap::{Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity};

/// Runs the local Warframe Observer application.
#[derive(Debug, Parser)]
#[command(name = "wf-observer", version, arg_required_else_help = true)]
pub(crate) struct Cli {
    #[command(flatten)]
    pub verbosity: Verbosity<InfoLevel>,
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
    Run {
        /// Prints a connection ticket after the transport has bound.
        #[arg(long)]
        print_ticket: bool,
        /// Stops after the process supervising standard input closes it.
        #[arg(long)]
        shutdown_on_stdin_close: bool,
    },
    /// Prints the service's stable Iroh endpoint identifier.
    Endpoint,
}
