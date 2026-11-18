use std::path::PathBuf;
use std::sync::atomic::Ordering;

use clap::builder::styling::{AnsiColor, Styles};
use clap::{Parser, Subcommand};

/// MPV daemon controller
#[derive(Parser, Debug)]
#[command(version)]
#[command(styles = CLAP_STYLING)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Print debug IPC messages
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Start the MPV daemon if it isn't already running
    Start,
    /// Print running daemon process id
    Pid,
    /// Kill the running daemon
    Kill,
    /// Send IPC command to MPV
    Send {
        /// MPV IPC command
        command: Vec<String>,
    },
    /// Observe MPV property
    Observe {
        /// Property to observe
        property: String,
    },
    /// Print (current) playlist
    #[command(alias = "ls")]
    List {
        /// Print without decorations
        #[arg(short, long)]
        plain: bool,
        /// Print with absolute paths
        #[arg(short, long)]
        full: bool,
        /// Playlist XSPF file path to print instead
        playlist: Option<PathBuf>,
    },
    /// Play (at index)
    Play {
        /// Optional index
        index: Option<usize>,
        /// Playlist XSPF file to load and play
        #[arg(short, long)]
        playlist: Option<PathBuf>,
    },
    /// Pause playback
    #[command(alias = "pause")]
    Stop,
    /// Append to (current) playlist
    #[command(alias = "push")]
    Add {
        /// File paths to add
        files: Vec<PathBuf>,
        /// Playlist XSPF file path to add to instead
        #[arg(short, long)]
        playlist: Option<PathBuf>,
    },
    /// Clear current playlist
    Clear,
    /// Remove track from (current) playlist
    #[command(alias = "rm")]
    Remove {
        /// Track index to remove
        index: usize,
        /// Playlist XSPF file path to remove from instead
        #[arg(short, long)]
        playlist: Option<PathBuf>,
    },
    /// Move track within (current) playlist
    #[command(alias = "mv")]
    Move {
        /// Source index (1-based)
        from: usize,
        /// Destination index (1-based)
        to: usize,
        /// Playlist XSPF file path to modify instead
        #[arg(short, long)]
        playlist: Option<PathBuf>,
    },
    /// Go to next track in queue
    Next,
    /// Go to previous track in queue
    #[command(alias = "prev")]
    Previous,
}

pub const CLAP_STYLING: Styles = Styles::styled()
    .header(AnsiColor::White.on_default())
    .usage(AnsiColor::White.on_default());

fn main() {
    let cli = Cli::parse();
    if cli.verbose {
        mpvd::ipc::DEBUG.store(true, Ordering::Relaxed);
    }
    match cli.command {
        None => {}
        Some(Command::Start) => {
            try_fail(mpvd::daemon::start());
        }
        Some(Command::Pid) => {
            if let Some(pid) = mpvd::daemon::get_pid() {
                println!("{pid}");
            } else {
                eprintln!("error: mpvd is not running");
                std::process::exit(1);
            }
        }
        Some(Command::Kill) => {
            try_fail(mpvd::daemon::kill());
        }
        Some(Command::Send { command }) => {
            let args: Vec<serde_json::Value> =
                command.iter().map(|a| mpvd::ipc::parse_arg(a)).collect();
            match mpvd::ipc::send_raw(&args) {
                Ok(value) => println!("{value}"),
                Err(msg) => {
                    eprintln!("error: {msg}");
                    std::process::exit(1);
                }
            }
        }
        Some(Command::Observe { property }) => {
            try_fail(mpvd::ipc::observe(&property, |value| println!("{value}")));
        }
        Some(Command::List {
            playlist,
            plain,
            full,
        }) => {
            let result = if let Some(path) = playlist {
                mpvd::playlist::print_playlist(&path, plain, full)
            } else {
                mpvd::playlist::print_current_playlist(plain, full)
            };
            try_fail(result);
        }
        Some(Command::Play { index, playlist }) => {
            let mut result: Result<(), String>;
            if let Some(playlist) = playlist {
                result = mpvd::control::clear_playlist()
                    .and_then(|_| mpvd::control::push_to_playlist(&playlist.to_string_lossy()));
                if let Some(index) = index {
                    result = result.and_then(|_| mpvd::control::play_at_index(index));
                }
            } else {
                result = match index {
                    Some(index) => mpvd::control::play_at_index(index),
                    None => mpvd::control::set_pause(false),
                };
            }
            try_fail(result);
        }
        Some(Command::Stop) => {
            try_fail(mpvd::control::set_pause(true));
        }
        Some(Command::Add { files, playlist }) => {
            let mut code = 0;
            if let Some(playlist) = playlist {
                if let Err(msg) = mpvd::playlist::push_to_file(&playlist, &files) {
                    code += 1;
                    eprintln!("error: {msg}");
                }
            } else {
                let _ = mpvd::daemon::start();
                for file in files {
                    if let Err(msg) = mpvd::control::push_to_playlist(&file.to_string_lossy()) {
                        code += 1;
                        eprintln!("error: {msg}");
                    }
                }
            }
            std::process::exit(code)
        }
        Some(Command::Clear) => {
            try_fail(mpvd::control::clear_playlist());
        }
        Some(Command::Remove { index, playlist }) => {
            try_fail(match playlist {
                Some(playlist) => mpvd::playlist::remove_from_file(&playlist, index),
                None => mpvd::control::remove_from_playlist(index),
            });
        }
        Some(Command::Next) => {
            try_fail(mpvd::control::go_next());
        }
        Some(Command::Previous) => {
            try_fail(mpvd::control::go_previous());
        }
        Some(Command::Move { from, to, playlist }) => {
            try_fail(match playlist {
                Some(playlist) => mpvd::playlist::move_in_file(&playlist, from, to),
                None => mpvd::control::move_in_playlist(from, to),
            });
        }
    }
}

fn try_fail<T>(result: Result<T, String>) {
    if let Err(msg) = result {
        eprintln!("error: {msg}");
        std::process::exit(1);
    }
}
