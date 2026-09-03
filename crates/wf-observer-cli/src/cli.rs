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
    /// Attaches a background agent to the running Warframe process.
    Attach,
    /// Reports the current background agent and target.
    Status,
    /// Runs the targetless transport used by integration tests.
    #[command(name = "_serve", hide = true)]
    Serve {
        /// Prints a connection ticket after the transport has bound.
        #[arg(long)]
        print_ticket: bool,
        /// Stops after the process supervising standard input closes it.
        #[arg(long)]
        shutdown_on_stdin_close: bool,
    },
    /// Runs the internal target-bound background process.
    #[command(name = "_agent", hide = true)]
    Agent {
        /// Target process identifier selected by the invoking CLI.
        #[arg(long)]
        pid: u32,
        /// Platform-specific target process creation marker.
        #[arg(long)]
        start_marker: u64,
    },
}
