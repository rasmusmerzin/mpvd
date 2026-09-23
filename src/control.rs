use serde::Deserialize;
use serde_json::json;

use crate::daemon;
use crate::ipc;
use crate::playlist::read_playlist;

#[derive(Debug, Deserialize)]
pub struct PlaylistItem {
    pub filename: String,
    pub current: Option<bool>,
}

pub fn get_playlist() -> Result<Vec<PlaylistItem>, String> {
    let data = ipc::send(&[json!("get_property"), json!("playlist")])?;
    serde_json::from_value(data).map_err(|e| format!("parse error: {e}"))
}

pub fn get_pause() -> Result<bool, String> {
    let data = ipc::send(&[json!("get_property"), json!("pause")])?;
    data.as_bool().ok_or("expected bool".into())
}

pub fn push_to_playlist(file: &str) -> Result<(), String> {
    daemon::start();
    let path = crate::config::resolve_tilde(file);
    if let Some(files) = read_playlist(&path) {
        for filepath in &files {
            ipc::send(&[
                json!("loadfile"),
                json!(filepath.to_string_lossy()),
                json!("append-play"),
            ])?;
        }
    } else {
        ipc::send(&[
            json!("loadfile"),
            json!(path.to_string_lossy()),
            json!("append-play"),
        ])?;
    }
    Ok(())
}

pub fn insert_next(file: &str) -> Result<(), String> {
    daemon::start();
    let path = crate::config::resolve_tilde(file);
    if let Some(mut files) = read_playlist(&path) {
        files.reverse();
        for filepath in &files {
            ipc::send(&[
                json!("loadfile"),
                json!(filepath.to_string_lossy()),
                json!("insert-next"),
            ])?;
        }
    } else {
        ipc::send(&[
            json!("loadfile"),
            json!(path.to_string_lossy()),
            json!("insert-next"),
        ])?;
    }
    Ok(())
}

pub fn set_pause(paused: bool) -> Result<(), String> {
    ipc::send(&[json!("set_property"), json!("pause"), json!(paused)])?;
    Ok(())
}

pub fn play_at_index(index: usize) -> Result<(), String> {
    ipc::send(&[json!("playlist-play-index"), json!(index - 1)])?;
    Ok(())
}

pub fn go_next() -> Result<(), String> {
    ipc::send(&[json!("playlist-next")])?;
    Ok(())
}

pub fn go_prev() -> Result<(), String> {
    ipc::send(&[json!("playlist-prev")])?;
    Ok(())
}

pub fn move_in_playlist(from: usize, to: usize) -> Result<(), String> {
    if from == to {
        return Ok(());
    }
    if from < to {
        ipc::send(&[json!("playlist-move"), json!(from - 1), json!(to)])?;
    } else {
        ipc::send(&[json!("playlist-move"), json!(from - 1), json!(to - 1)])?;
    }
    Ok(())
}

pub fn remove_from_playlist(index: usize) -> Result<(), String> {
    ipc::send(&[json!("playlist-remove"), json!(index - 1)])?;
    Ok(())
}

pub fn get_position() -> Result<usize, String> {
    let data = ipc::send(&[json!("get_property"), json!("playlist-pos")])?;
    let pos = data.as_u64().ok_or("expected number")?;
    Ok(pos as usize + 1)
}

pub fn get_time() -> Result<f64, String> {
    let data = ipc::send(&[json!("get_property"), json!("time-pos")])?;
    data.as_f64().ok_or("expected number".into())
}

pub fn get_duration() -> Result<f64, String> {
    let data = ipc::send(&[json!("get_property"), json!("duration")])?;
    data.as_f64().ok_or("expected number".into())
}

pub fn format_time_string(time: f64, duration: f64) -> String {
    let pos_secs = time as i64;
    let dur_secs = duration as i64;
    let mm = format!("{:02}", pos_secs / 60);
    let ss = format!("{:02}", pos_secs % 60);
    let mm_dur = format!("{:02}", dur_secs / 60);
    let ss_dur = format!("{:02}", dur_secs % 60);
    format!("{mm}:{ss}/{mm_dur}:{ss_dur}")
}

pub fn seek(seconds: f64) -> Result<(), String> {
    ipc::send(&[json!("seek"), json!(seconds), json!("relative")])?;
    Ok(())
}

pub fn get_state() -> Result<&'static str, String> {
    let paused = get_pause()?;
    Ok(if paused { "paused" } else { "playing" })
}

pub fn get_current() -> Result<String, String> {
    let pos_data = ipc::send(&[json!("get_property"), json!("playlist-pos")])?;
    let pos = pos_data.as_i64().ok_or("expected number")?;
    if pos < 0 {
        return Err("no current track".into());
    }
    let playlist = get_playlist()?;
    playlist
        .get(pos as usize)
        .map(|item| item.filename.clone())
        .ok_or("no current track".into())
}

pub fn display_name(filename: &str, absolute: bool) -> &str {
    if absolute {
        filename
    } else {
        filename.rsplit('/').next().unwrap_or(filename)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{FakeServer, Temp, handler_fn, mpv_ok};
    use serde_json::{Value, json};
    use std::path::Path;

    fn mk_xspf(dir: &Path, name: &str, tracks: &[&str]) -> std::path::PathBuf {
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

    fn two_track_playlist() -> Value {
        json!([
            { "filename": "/music/a.mp3", "current": true },
            { "filename": "/music/b.mp3", "current": false },
        ])
    }

    fn default_handler() -> crate::test_util::MpvHandler {
        handler_fn(|req| {
            let cmd = req
                .get("command")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            let first = cmd.first().and_then(|v| v.as_str()).unwrap_or("");
            match first {
                "get_property" => match cmd.get(1).and_then(|v| v.as_str()).unwrap_or("") {
                    "playlist" => mpv_ok(two_track_playlist()),
                    "playlist-pos" => mpv_ok(json!(1)),
                    "time-pos" => mpv_ok(json!(61.5)),
                    "duration" => mpv_ok(json!(200.0)),
                    "pause" => mpv_ok(json!(false)),
                    _ => mpv_ok(Value::Null),
                },
                _ => mpv_ok(Value::Null),
            }
        })
    }

    fn received_cmds(server: &FakeServer, expected: usize) -> Vec<Value> {
        crate::test_util::wait_commands(&server.received, expected)
    }

    #[test]
    fn get_playlist_success() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            let playlist = get_playlist().unwrap();
            assert_eq!(playlist.len(), 2);
            assert_eq!(playlist[0].filename, "/music/a.mp3");
            assert_eq!(playlist[0].current, Some(true));
        });
    }

    #[test]
    fn get_playlist_parse_error() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!({ "x": 1 }))));
        server.with_env(|| {
            assert!(get_playlist().unwrap_err().contains("parse error"));
        });
    }

    #[test]
    fn get_pause_variants() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!(true))));
        server.with_env(|| assert!(get_pause().unwrap()));
    }

    #[test]
    fn get_pause_wrong_type_errors() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!("yes"))));
        server.with_env(|| {
            assert!(get_pause().unwrap_err().contains("expected bool"));
        });
    }

    #[test]
    fn push_plain_file_appends() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            push_to_playlist("/music/song.mp3").unwrap();
        });
        let cmds = received_cmds(&server, 1);
        assert_eq!(cmds[0][0], "loadfile");
        assert_eq!(cmds[0][1], "/music/song.mp3");
        assert_eq!(cmds[0][2], "append-play");
    }

    #[test]
    fn push_playlist_file_expands_tracks() {
        let server = FakeServer::start(default_handler());
        let dir = Temp::new("push");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        let track = dir.join("music/one.flac");
        std::fs::write(&track, b"x").unwrap();
        let pl = mk_xspf(dir.path(), "sets.xspf", &["music/one.flac"]);
        let pl_str = pl.to_string_lossy().to_string();
        server.with_env(|| {
            push_to_playlist(&pl_str).unwrap();
        });
        let cmds = received_cmds(&server, 1);
        assert_eq!(cmds[0][0], "loadfile");
        assert_eq!(
            cmds[0][1],
            dir.join("music/one.flac").to_string_lossy().to_string()
        );
        assert_eq!(cmds[0][2], "append-play");
    }

    #[test]
    fn insert_plain_file_next() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            insert_next("/music/song.mp3").unwrap();
        });
        let cmds = received_cmds(&server, 1);
        assert_eq!(cmds[0][0], "loadfile");
        assert_eq!(cmds[0][2], "insert-next");
    }

    #[test]
    fn insert_playlist_file_reverses_order() {
        let server = FakeServer::start(default_handler());
        let dir = Temp::new("insert");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        std::fs::write(dir.join("music/a.flac"), b"x").unwrap();
        std::fs::write(dir.join("music/b.flac"), b"x").unwrap();
        let pl = mk_xspf(dir.path(), "sets.xspf", &["music/a.flac", "music/b.flac"]);
        let pl_str = pl.to_string_lossy().to_string();
        server.with_env(|| {
            insert_next(&pl_str).unwrap();
        });
        let cmds = received_cmds(&server, 2);
        assert_eq!(cmds.len(), 2);
        assert!(cmds[0][1].as_str().unwrap().ends_with("music/b.flac"));
        assert!(cmds[1][1].as_str().unwrap().ends_with("music/a.flac"));
    }

    #[test]
    fn set_pause_sends_command() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            set_pause(true).unwrap();
        });
        assert_eq!(
            received_cmds(&server, 1)[0],
            json!(["set_property", "pause", true])
        );
    }

    #[test]
    fn play_at_index_converts_to_zero_based() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            play_at_index(3).unwrap();
        });
        assert_eq!(
            received_cmds(&server, 1)[0],
            json!(["playlist-play-index", 2])
        );
    }

    #[test]
    fn go_next_and_prev() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            go_next().unwrap();
            go_prev().unwrap();
        });
        let cmds = received_cmds(&server, 2);
        assert_eq!(cmds[0][0], "playlist-next");
        assert_eq!(cmds[1][0], "playlist-prev");
    }

    #[test]
    fn move_in_playlist_same_index_is_noop() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            move_in_playlist(2, 2).unwrap();
        });
        assert!(server.received.try_iter().next().is_none());
    }

    #[test]
    fn move_in_playlist_forward() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            move_in_playlist(2, 4).unwrap();
        });
        assert_eq!(received_cmds(&server, 1)[0], json!(["playlist-move", 1, 4]));
    }

    #[test]
    fn move_in_playlist_backward() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            move_in_playlist(4, 2).unwrap();
        });
        assert_eq!(received_cmds(&server, 1)[0], json!(["playlist-move", 3, 1]));
    }

    #[test]
    fn remove_from_playlist_sends_zero_based() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            remove_from_playlist(1).unwrap();
        });
        assert_eq!(received_cmds(&server, 1)[0], json!(["playlist-remove", 0]));
    }

    #[test]
    fn get_position_is_one_based() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            assert_eq!(get_position().unwrap(), 2);
        });
    }

    #[test]
    fn get_position_wrong_type_errors() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!("x"))));
        server.with_env(|| {
            assert!(get_position().unwrap_err().contains("expected number"));
        });
    }

    #[test]
    fn get_time_and_duration() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            assert_eq!(get_time().unwrap(), 61.5);
            assert_eq!(get_duration().unwrap(), 200.0);
        });
    }

    #[test]
    fn get_time_wrong_type_errors() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!("x"))));
        server.with_env(|| {
            assert!(get_time().unwrap_err().contains("expected number"));
            assert!(get_duration().unwrap_err().contains("expected number"));
        });
    }

    #[test]
    fn format_time_string_cases() {
        assert_eq!(format_time_string(61.5, 200.0), "01:01/03:20");
        assert_eq!(format_time_string(0.0, 0.0), "00:00/00:00");
        assert_eq!(format_time_string(3599.9, 3661.7), "59:59/61:01");
    }

    #[test]
    fn seek_sends_relative_command() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            seek(5.0).unwrap();
        });
        assert_eq!(
            received_cmds(&server, 1)[0],
            json!(["seek", 5.0, "relative"])
        );
    }

    #[test]
    fn get_state_reflects_pause() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!(true))));
        server.with_env(|| assert_eq!(get_state().unwrap(), "paused"));

        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!(false))));
        server.with_env(|| assert_eq!(get_state().unwrap(), "playing"));
    }

    #[test]
    fn get_current_returns_track_at_position() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            assert_eq!(get_current().unwrap(), "/music/b.mp3");
        });
    }

    #[test]
    fn get_current_errors_without_current() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!(-1))));
        server.with_env(|| {
            assert!(get_current().unwrap_err().contains("no current track"));
        });
    }

    #[test]
    fn get_current_errors_out_of_range() {
        let server = FakeServer::start(handler_fn(|req| {
            let cmd = req
                .get("command")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            let prop = cmd.get(1).and_then(|v| v.as_str()).unwrap_or("");
            match prop {
                "playlist-pos" => mpv_ok(json!(9)),
                "playlist" => mpv_ok(two_track_playlist()),
                _ => mpv_ok(Value::Null),
            }
        }));
        server.with_env(|| {
            assert!(get_current().unwrap_err().contains("no current track"));
        });
    }

    #[test]
    fn display_name_variants() {
        assert_eq!(display_name("/a/b/c.mp3", false), "c.mp3");
        assert_eq!(display_name("/a/b/c.mp3", true), "/a/b/c.mp3");
        assert_eq!(display_name("single", false), "single");
    }
}
