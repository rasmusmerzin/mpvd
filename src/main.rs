mod config;
mod control;
mod daemon;
mod find;
mod interactive;
mod ipc;
mod list;
mod pick;
mod playlist;
mod term;
#[cfg(test)]
mod test_util;

use clap::{Parser, Subcommand};
use std::fmt::Display;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "mpvd", version, about = "MPV daemon control")]
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
        /// Optional XSPF playlist file path.
        file: Option<PathBuf>,
    },
    /// Append one or more files to the playlist
    Push {
        /// Files to append
        files: Vec<PathBuf>,
        /// XSPF playlist file path to push to instead of the current playlist
        #[arg(short, long)]
        playlist: Option<PathBuf>,
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
        /// XSPF playlist file path to move within instead of the current playlist
        #[arg(short, long)]
        playlist: Option<PathBuf>,
    },
    /// Remove a track from the playlist
    #[command(alias = "rm")]
    Remove {
        /// Playlist index to remove (1-based)
        index: usize,
        /// XSPF playlist file path to remove from instead of the current playlist
        #[arg(short, long)]
        playlist: Option<PathBuf>,
    },
    /// Print playlist index of the current track
    #[command(alias = "pos")]
    Position,
    /// Print current track time position or file duration
    Time {
        /// Print seconds
        #[arg(short, long)]
        seconds: bool,
        /// Print duration without formatting
        #[arg(short, long)]
        duration: bool,
        /// Optional file to probe duration for
        file: Option<PathBuf>,
    },
    /// Print playing/paused state
    State,
    /// Print current track
    Current,
    /// Export the playlist as XSPF
    Export {
        /// Output path
        output: PathBuf,
        /// Print to stdout instead of writing to file
        #[arg(short, long)]
        print: bool,
        /// Overwrite target path
        #[arg(short, long)]
        force: bool,
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

fn time_string(seconds: bool, duration: bool, file: Option<PathBuf>) -> Result<String, String> {
    if let Some(filepath) = file {
        let path = config::resolve_tilde(&filepath.to_string_lossy());
        let is_playlist = path.extension().and_then(|e| e.to_str()) == Some("xspf");
        let files = if is_playlist {
            playlist::read_playlist(&path).ok_or("Unable to read playlist")?
        } else {
            vec![path]
        };
        let mut total = 0.0;
        for filepath in &files {
            let file_str = filepath.to_string_lossy().to_string();
            let d = control::probe_duration(&file_str)
                .ok_or_else(|| format!("failed to probe duration: {file_str}"))?;
            total += d;
        }
        return Ok(if duration {
            total.to_string()
        } else {
            control::format_time(total)
        });
    }
    match (seconds, duration) {
        (false, false) => control::get_time()
            .and_then(|t| control::get_duration().map(|d| control::format_time_string(t, d))),
        (true, false) => control::get_time().map(|t| t.to_string()),
        (false, true) => control::get_duration().map(|d| d.to_string()),
        (true, true) => {
            control::get_time().and_then(|t| control::get_duration().map(|d| format!("{t}/{d}")))
        }
    }
}

fn play(index: Option<usize>) -> Result<(), String> {
    if let Some(i) = index {
        control::play_at_index(i)?;
    }
    control::set_pause(false)
}

fn run(cli: Cli) -> ExitCode {
    match cli.command {
        None => {
            interactive::run();
            ExitCode::SUCCESS
        }
        Some(Commands::Init) => {
            let code = daemon::start();
            if code == ExitCode::from(2) {
                eprintln!("mpv daemon is already running");
            }
            code
        }
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
            file,
        }) => match file {
            Some(path) => run_result(playlist::print_playlist(&path, plain, full)),
            None => {
                if interactive {
                    interactive::run();
                    ExitCode::SUCCESS
                } else {
                    run_result(playlist::print_current_playlist(plain, full))
                }
            }
        },
        Some(Commands::Push {
            files,
            playlist: target,
        }) => run_result(match target {
            Some(path) => playlist::push_to_file(&path, &files),
            None => files
                .iter()
                .try_for_each(|f| control::push_to_playlist(&f.to_string_lossy())),
        }),
        Some(Commands::Insert { files }) => {
            run_result(files.iter().rev().try_for_each(|f| control::insert_next(f)))
        }
        Some(Commands::Move {
            from,
            to,
            playlist: target,
        }) => run_result(match target {
            Some(path) => playlist::move_in_file(&path, from, to),
            None => control::move_in_playlist(from, to),
        }),
        Some(Commands::Remove {
            index,
            playlist: target,
        }) => run_result(match target {
            Some(path) => playlist::remove_from_file(&path, index),
            None => control::remove_from_playlist(index),
        }),
        Some(Commands::Position) => print_result(control::get_position()),
        Some(Commands::Time {
            seconds,
            duration,
            file,
        }) => print_result(time_string(seconds, duration, file)),
        Some(Commands::State) => print_result(control::get_state()),
        Some(Commands::Current) => print_result(control::get_current()),
        Some(Commands::Export {
            output,
            print,
            force,
        }) => run_result(playlist::export_playlist(&output, print, force)),
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

fn main() -> ExitCode {
    // fix piping output into `head`
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    run(Cli::parse())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{FakeServer, Temp, handler_fn, mpv_ok};
    use serde_json::{Value, json};
    use std::path::PathBuf;

    fn command(cmd: Commands) -> Cli {
        Cli { command: Some(cmd) }
    }

    /// Default handler answering the get_property family used by CLI commands.
    fn default_handler() -> crate::test_util::MpvHandler {
        handler_fn(|req| {
            let cmd = req
                .get("command")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            if cmd.first().and_then(|v| v.as_str()).unwrap_or("") == "get_property" {
                match cmd.get(1).and_then(|v| v.as_str()).unwrap_or("") {
                    "playlist" => mpv_ok(json!([
                        { "filename": "/music/a.mp3", "current": true },
                        { "filename": "/music/b.mp3", "current": false },
                    ])),
                    "playlist-pos" => mpv_ok(json!(1)),
                    "time-pos" => mpv_ok(json!(61.5)),
                    "duration" => mpv_ok(json!(200.0)),
                    "pause" => mpv_ok(json!(false)),
                    _ => mpv_ok(Value::Null),
                }
            } else {
                mpv_ok(Value::Null)
            }
        })
    }

    fn mk_xspf(dir: &Temp, name: &str, tracks: &[&str]) -> PathBuf {
        let mut pl = xspf::Playlist::default().clone();
        for t in tracks {
            pl.add_track(
                xspf::Track::default()
                    .add_location(t.to_string())
                    .set_title(t.to_string()),
            );
        }
        let path = dir.join(name);
        std::fs::write(&path, pl.to_string_pretty("\t")).unwrap();
        path
    }

    #[test]
    fn cli_parses_subcommands() {
        let cli =
            Cli::try_parse_from(["mpvd", "list", "--plain", "--interactive", "--full"]).unwrap();
        match cli.command.unwrap() {
            Commands::List {
                plain,
                full,
                interactive,
                file,
            } => {
                assert!(plain && full && interactive);
                assert!(file.is_none());
            }
            _ => panic!("expected List"),
        }
        let cli = Cli::try_parse_from(["mpvd", "time", "--seconds", "--duration"]).unwrap();
        match cli.command.unwrap() {
            Commands::Time {
                seconds,
                duration,
                file,
            } => {
                assert!(seconds && duration);
                assert!(file.is_none());
            }
            _ => panic!("expected Time"),
        }
        let cli = Cli::try_parse_from(["mpvd", "time", "-d", "song.mp3"]).unwrap();
        match cli.command.unwrap() {
            Commands::Time { duration, file, .. } => {
                assert!(duration);
                assert_eq!(file, Some(PathBuf::from("song.mp3")));
            }
            _ => panic!("expected Time"),
        }
        let cli = Cli::try_parse_from(["mpvd", "send", "get_property", "duration"]).unwrap();
        match cli.command.unwrap() {
            Commands::Send { cmd } => assert_eq!(cmd, vec!["get_property", "duration"]),
            _ => panic!("expected Send"),
        }
    }

    #[test]
    fn run_env_prints_paths() {
        let dir = Temp::new("main");
        let sock = dir.sock().to_string_lossy().to_string();
        let pidf = dir.pid().to_string_lossy().to_string();
        crate::test_util::with_env(
            &[
                ("MPVD_SOCK", Some(sock.as_str())),
                ("MPVD_PID", Some(pidf.as_str())),
            ],
            || {
                assert_eq!(run(command(Commands::Env)), ExitCode::SUCCESS);
            },
        );
    }

    #[test]
    fn run_init_already_running() {
        let dir = Temp::new("main");
        std::fs::write(dir.pid(), format!("{}\n", std::process::id())).unwrap();
        let sock = dir.sock().to_string_lossy().to_string();
        let pidf = dir.pid().to_string_lossy().to_string();
        crate::test_util::with_env(
            &[
                ("MPVD_SOCK", Some(sock.as_str())),
                ("MPVD_PID", Some(pidf.as_str())),
            ],
            || {
                assert_eq!(run(command(Commands::Init)), ExitCode::from(2));
            },
        );
    }

    #[test]
    fn run_pid_and_kill_without_daemon_error() {
        let dir = Temp::new("main");
        let sock = dir.sock().to_string_lossy().to_string();
        let pidf = dir.pid().to_string_lossy().to_string();
        crate::test_util::with_env(
            &[
                ("MPVD_SOCK", Some(sock.as_str())),
                ("MPVD_PID", Some(pidf.as_str())),
            ],
            || {
                assert_eq!(run(command(Commands::Pid)), ExitCode::from(1));
                assert_eq!(run(command(Commands::Kill)), ExitCode::from(1));
            },
        );
    }

    #[test]
    fn run_pid_shows_live_pid() {
        let dir = Temp::new("main");
        std::fs::write(dir.pid(), format!("{}\n", std::process::id())).unwrap();
        let sock = dir.sock().to_string_lossy().to_string();
        let pidf = dir.pid().to_string_lossy().to_string();
        crate::test_util::with_env(
            &[
                ("MPVD_SOCK", Some(sock.as_str())),
                ("MPVD_PID", Some(pidf.as_str())),
            ],
            || {
                assert_eq!(run(command(Commands::Pid)), ExitCode::SUCCESS);
            },
        );
    }

    #[test]
    fn run_list_file_roundtrip() {
        let dir = Temp::new("main");
        let pl = mk_xspf(&dir, "list.xspf", &["tracks/song.flac"]);
        let cli = command(Commands::List {
            plain: true,
            full: false,
            interactive: false,
            file: Some(pl),
        });
        assert_eq!(run(cli), ExitCode::SUCCESS);
    }

    #[test]
    fn run_list_file_missing_errors() {
        let cli = command(Commands::List {
            plain: true,
            full: false,
            interactive: false,
            file: Some(PathBuf::from("/nonexistent/missing.xspf")),
        });
        assert_eq!(run(cli), ExitCode::from(1));
    }

    #[test]
    fn run_push_to_file_creates_playlist() {
        let dir = Temp::new("main");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        std::fs::write(dir.join("music/t.flac"), b"x").unwrap();
        let target = dir.join("out.xspf");
        let cli = command(Commands::Push {
            files: vec![dir.join("music/t.flac")],
            playlist: Some(target.clone()),
        });
        assert_eq!(run(cli), ExitCode::SUCCESS);
        assert!(target.exists());
    }

    #[test]
    fn run_move_and_remove_in_file() {
        let dir = Temp::new("main");
        let pl = mk_xspf(&dir, "pl.xspf", &["a.flac", "b.flac", "c.flac"]);
        assert_eq!(
            run(command(Commands::Move {
                from: 1,
                to: 3,
                playlist: Some(pl.clone()),
            })),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run(command(Commands::Remove {
                index: 2,
                playlist: Some(pl.clone()),
            })),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn run_control_commands_with_fake_mpv() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            assert_eq!(run(command(Commands::Position)), ExitCode::SUCCESS);
            assert_eq!(
                run(command(Commands::Time {
                    seconds: false,
                    duration: false,
                    file: None,
                })),
                ExitCode::SUCCESS
            );
            assert_eq!(
                run(command(Commands::Time {
                    seconds: true,
                    duration: false,
                    file: None,
                })),
                ExitCode::SUCCESS
            );
            assert_eq!(
                run(command(Commands::Time {
                    seconds: false,
                    duration: true,
                    file: None,
                })),
                ExitCode::SUCCESS
            );
            assert_eq!(
                run(command(Commands::Time {
                    seconds: true,
                    duration: true,
                    file: None,
                })),
                ExitCode::SUCCESS
            );
            assert_eq!(run(command(Commands::State)), ExitCode::SUCCESS);
            assert_eq!(run(command(Commands::Current)), ExitCode::SUCCESS);
            assert_eq!(
                run(command(Commands::Play { index: Some(2) })),
                ExitCode::SUCCESS
            );
            assert_eq!(
                run(command(Commands::Play { index: None })),
                ExitCode::SUCCESS
            );
            assert_eq!(run(command(Commands::Stop)), ExitCode::SUCCESS);
            assert_eq!(run(command(Commands::Next)), ExitCode::SUCCESS);
            assert_eq!(run(command(Commands::Prev)), ExitCode::SUCCESS);
            assert_eq!(
                run(command(Commands::Push {
                    files: vec![PathBuf::from("/music/song.mp3")],
                    playlist: None,
                })),
                ExitCode::SUCCESS
            );
            assert_eq!(
                run(command(Commands::Insert {
                    files: vec!["/music/song.mp3".into()],
                })),
                ExitCode::SUCCESS
            );
            assert_eq!(
                run(command(Commands::Move {
                    from: 1,
                    to: 2,
                    playlist: None,
                })),
                ExitCode::SUCCESS
            );
            assert_eq!(
                run(command(Commands::Remove {
                    index: 1,
                    playlist: None,
                })),
                ExitCode::SUCCESS
            );
        });
    }

    #[test]
    fn run_time_with_file_probes_duration() {
        let dir = Temp::new("main");
        let file = dir.join("tone.wav");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-y",
            ])
            .arg(&file)
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            run(command(Commands::Time {
                seconds: false,
                duration: false,
                file: Some(file.clone()),
            })),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run(command(Commands::Time {
                seconds: false,
                duration: true,
                file: Some(file.clone()),
            })),
            ExitCode::SUCCESS
        );
        let missing = dir.join("missing.mp3");
        assert_eq!(
            run(command(Commands::Time {
                seconds: false,
                duration: false,
                file: Some(missing),
            })),
            ExitCode::from(1)
        );
    }

    #[test]
    fn run_time_with_playlist_sums_durations() {
        let dir = Temp::new("main");
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-y",
            ])
            .arg(dir.join("tone1.wav"))
            .status()
            .unwrap();
        assert!(status.success());
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=3",
                "-y",
            ])
            .arg(dir.join("tone2.wav"))
            .status()
            .unwrap();
        assert!(status.success());
        let pl = mk_xspf(&dir, "list.xspf", &["tone1.wav", "tone2.wav"]);
        assert_eq!(
            run(command(Commands::Time {
                seconds: false,
                duration: false,
                file: Some(pl.clone()),
            })),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run(command(Commands::Time {
                seconds: false,
                duration: true,
                file: Some(pl),
            })),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn run_send_and_observe() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            assert_eq!(
                run(command(Commands::Send {
                    cmd: vec!["get_property".into(), "pause".into()],
                })),
                ExitCode::SUCCESS
            );
        });
        let server = FakeServer::start(std::sync::Arc::new(|_| {
            crate::test_util::Reply::CloseAfter(json!({
                "event": "property-change",
                "id": 1,
                "name": "pause",
                "data": false,
            }))
        }));
        server.with_env(|| {
            assert_eq!(
                run(command(Commands::Observe {
                    property: "pause".into()
                })),
                ExitCode::SUCCESS
            );
        });
    }

    #[test]
    fn run_export_to_file() {
        let dir = Temp::new("main");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        let a = dir.join("music/a.mp3").to_string_lossy().to_string();
        let b = dir.join("music/b.mp3").to_string_lossy().to_string();
        let server = FakeServer::start(handler_fn(move |req| {
            let cmd = req
                .get("command")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            match cmd.first().and_then(|v| v.as_str()).unwrap_or("") {
                "get_property" => match cmd.get(1).and_then(|v| v.as_str()).unwrap_or("") {
                    "playlist" => mpv_ok(json!([
                        { "filename": a, "current": true },
                        { "filename": b, "current": false },
                    ])),
                    _ => mpv_ok(Value::Null),
                },
                _ => mpv_ok(Value::Null),
            }
        }));
        let out = dir.join("out.xspf");
        server.with_env(|| {
            assert_eq!(
                run(command(Commands::Export {
                    output: out.clone(),
                    print: false,
                    force: false,
                })),
                ExitCode::SUCCESS
            );
            assert!(out.exists());
        });
    }

    #[test]
    fn run_pick_empty_dir() {
        let dir = Temp::new("main");
        assert_eq!(
            run(command(Commands::Pick {
                dirpath: dir.path().to_string_lossy().into(),
            })),
            ExitCode::SUCCESS
        );
    }

    #[test]
    fn short_command_aliases_parse() {
        assert!(Cli::try_parse_from(["mpvd", "start"]).is_ok());
        assert!(Cli::try_parse_from(["mpvd", "ls", "--plain"]).is_ok());
        assert!(Cli::try_parse_from(["mpvd", "mv", "1", "2"]).is_ok());
        assert!(Cli::try_parse_from(["mpvd", "rm", "1"]).is_ok());
        assert!(Cli::try_parse_from(["mpvd", "pos"]).is_ok());
        assert!(Cli::try_parse_from(["mpvd", "previous"]).is_ok());
    }
}
