mod config;
mod control;
mod daemon;
mod find;
mod interactive;
mod ipc;
mod list;
mod pick;
mod playlist;

use clap::{Parser, Subcommand};
use std::fmt::Display;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "mpvd", about = "MPV daemon control")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
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
        /// Open interactive playlist
        #[arg(short, long)]
        interactive: bool,
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
    /// Pick files to playlist
    Pick {
        /// Directory path to browse
        #[arg(default_value = config::DEFAULT_MUSIC_DIR)]
        dirpath: String,
    },
}

fn print_result(result: Result<impl Display, String>) -> ExitCode {
    match result {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn run_result(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn time_string(seconds: bool, duration: bool) -> Result<String, String> {
    match (seconds, duration) {
        (false, false) => control::get_time()
            .and_then(|t| control::get_duration().map(|d| control::format_time_string(t, d))),
        (true, false) => control::get_time().map(|t| t.to_string()),
        (false, true) => control::get_duration().map(|d| d.to_string()),
        (true, true) => control::get_time()
            .and_then(|t| control::get_duration().map(|d| format!("{t}/{d}"))),
    }
}

fn play(index: Option<usize>) -> Result<(), String> {
    if let Some(i) = index {
        control::play_at_index(i)?;
    }
    control::set_pause(false)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        None => {
            interactive::run();
            ExitCode::SUCCESS
        }
        Some(Commands::Init) => daemon::start(),
        Some(Commands::Kill) => daemon::kill(),
        Some(Commands::Pid) => daemon::pid(),
        Some(Commands::Env) => {
            daemon::env();
            ExitCode::SUCCESS
        }
        Some(Commands::List {
            plain,
            full,
            interactive,
        }) => {
            if interactive {
                interactive::run();
                ExitCode::SUCCESS
            } else {
                run_result(playlist::print_playlist(plain, full))
            }
        }
        Some(Commands::Push { files }) => {
            run_result(files.iter().try_for_each(|f| control::push_to_playlist(f)))
        }
        Some(Commands::Insert { files }) => {
            run_result(files.iter().rev().try_for_each(|f| control::insert_next(f)))
        }
        Some(Commands::Move { from, to }) => run_result(control::move_in_playlist(from, to)),
        Some(Commands::Remove { index }) => run_result(control::remove_from_playlist(index)),
        Some(Commands::Position) => print_result(control::get_position()),
        Some(Commands::Time { seconds, duration }) => print_result(time_string(seconds, duration)),
        Some(Commands::State) => print_result(control::get_state()),
        Some(Commands::Current) => print_result(control::get_current()),
        Some(Commands::Play { index }) => run_result(play(index)),
        Some(Commands::Stop) => run_result(control::set_pause(true)),
        Some(Commands::Next) => run_result(control::go_next()),
        Some(Commands::Prev) => run_result(control::go_prev()),
        Some(Commands::Send { cmd }) => {
            let args: Vec<serde_json::Value> = cmd.iter().map(|a| ipc::parse_arg(a)).collect();
            print_result(ipc::send_raw(&args))
        }
        Some(Commands::Observe { property }) => run_result(ipc::observe(&property)),
        Some(Commands::Pick { dirpath }) => {
            pick::run(&dirpath);
            ExitCode::SUCCESS
        }
    }
}
