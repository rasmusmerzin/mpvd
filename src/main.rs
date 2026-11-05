use clap::builder::styling::{AnsiColor, Styles};
use clap::{Parser, Subcommand};

/// MPV daemon controller
#[derive(Parser, Debug)]
#[command(version)]
#[command(styles = CLAP_STYLING)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the MPV daemon if it isn't already running
    Start,
    /// Print running daemon process id
    Pid,
    /// Kill the running daemon
    Kill,
}

pub const CLAP_STYLING: Styles = Styles::styled()
    .header(AnsiColor::White.on_default())
    .usage(AnsiColor::White.on_default());

fn main() {
    match Cli::parse().command {
        None => {}
        Some(Command::Start) => {
            if let Err(msg) = mpvd::daemon::start() {
                eprintln!("error: {}", msg);
                std::process::exit(1);
            }
        }
        Some(Command::Pid) => {
            if let Some(pid) = mpvd::daemon::get_pid() {
                println!("{}", pid);
            } else {
                eprintln!("error: mpvd is not running");
                std::process::exit(1);
            }
        }
        Some(Command::Kill) => {
            if let Err(msg) = mpvd::daemon::kill() {
                eprintln!("error: {}", msg);
                std::process::exit(1);
            }
        }
    }
}
