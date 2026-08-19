mod config;
mod daemon;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "mpvd", about = "MPV daemon control")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start an idle mpv daemon in the background
    #[command(alias = "start")]
    Init,
    /// Kill the running mpv daemon
    Kill,
    /// Print the daemon's PID
    Pid,
    /// Print MPVD_SOCK and MPVD_PID paths
    Env,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => daemon::start(),
        Commands::Kill => daemon::kill(),
        Commands::Pid => daemon::pid(),
        Commands::Env => {
            daemon::env();
            ExitCode::SUCCESS
        }
    }
}
