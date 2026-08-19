mod config;
mod control;
mod daemon;
mod ipc;
mod playlist;

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
    /// Show the playlist
    #[command(alias = "ls")]
    List {
        /// Print without decorations
        #[arg(short, long)]
        plain: bool,
        /// Print with absolute paths
        #[arg(short, long)]
        full: bool,
    },
    /// Append one or more files to the playlist
    Push {
        /// Files to append
        files: Vec<String>,
    },
    /// Start/resume playback
    Play {
        /// Playlist index to play at (1-based)
        index: Option<usize>,
    },
    /// Pause playback
    Stop,
    /// Skip to the next track
    Next,
    /// Go to the previous track
    #[command(alias = "previous")]
    Prev,
    /// Send arbitrary command to the mpv IPC socket
    Send {
        /// JSON-native arguments (strings, numbers, booleans)
        cmd: Vec<String>,
    },
    /// Observe MPV property
    Observe {
        /// MPV property to observe
        property: String,
    },
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
        Commands::List { plain, full } => match playlist::print_playlist(plain, full) {
            Ok(out) => {
                if !out.is_empty() {
                    println!("{out}");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Commands::Push { files } => {
            for file in &files {
                if let Err(e) = control::push_to_playlist(file) {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            }
            ExitCode::SUCCESS
        }
        Commands::Play { index } => {
            if let Some(i) = index
                && let Err(e) = control::play_at_index(i)
            {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
            if let Err(e) = control::set_pause(false) {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Commands::Stop => match control::set_pause(true) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Commands::Next => match control::go_next() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Commands::Prev => match control::go_prev() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Commands::Send { cmd } => {
            let args: Vec<serde_json::Value> = cmd.iter().map(|a| ipc::parse_arg(a)).collect();
            match ipc::send_raw(&args) {
                Ok(resp) => {
                    println!("{resp}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            }
        }
        Commands::Observe { property } => match ipc::observe(&property) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
    }
}
