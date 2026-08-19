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
    /// Insert files to playlist after current track
    Insert {
        /// Files to insert
        files: Vec<String>,
    },
    /// Move a track within the playlist
    #[command(alias = "mv")]
    Move {
        /// Source index (1-based)
        from: usize,
        /// Destination index (1-based)
        to: usize,
    },
    /// Remove a track from the playlist
    #[command(alias = "rm")]
    Remove {
        /// Playlist index to remove (1-based)
        index: usize,
    },
    /// Print playlist index of the current track
    #[command(alias = "pos")]
    Position,
    /// Print current track time position
    Time {
        /// Print seconds
        #[arg(short, long)]
        seconds: bool,
        /// Print duration
        #[arg(short, long)]
        duration: bool,
    },
    /// Print playing/paused state
    State,
    /// Print current track
    Current,
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
        Commands::Insert { files } => {
            for file in files.iter().rev() {
                if let Err(e) = control::insert_next(file) {
                    eprintln!("{e}");
                    return ExitCode::from(1);
                }
            }
            ExitCode::SUCCESS
        }
        Commands::Move { from, to } => match control::move_in_playlist(from, to) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Commands::Remove { index } => match control::remove_from_playlist(index) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Commands::Position => match control::get_position() {
            Ok(pos) => {
                println!("{pos}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Commands::Time { seconds, duration } => match (seconds, duration) {
            (false, false) => match control::get_time()
                .and_then(|t| control::get_duration().map(|d| control::format_time_string(t, d)))
            {
                Ok(s) => {
                    println!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            },
            (true, false) => match control::get_time() {
                Ok(t) => {
                    println!("{t}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            },
            (false, true) => match control::get_duration() {
                Ok(d) => {
                    println!("{d}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            },
            (true, true) => match control::get_time()
                .and_then(|t| control::get_duration().map(|d| format!("{t}/{d}")))
            {
                Ok(s) => {
                    println!("{s}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            },
        },
        Commands::State => match control::get_state() {
            Ok(state) => {
                println!("{state}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
        Commands::Current => match control::get_current() {
            Ok(name) => {
                println!("{name}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        },
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
