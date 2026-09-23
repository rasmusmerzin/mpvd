use std::{
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
};
use xspf::{Playlist, Track};

use crate::control;

pub fn print_current_playlist(plain: bool, full: bool) -> Result<(), String> {
    let pause = control::get_pause()?;
    let playlist = control::get_playlist()?;
    let tty = std::io::stdout().is_terminal();
    for (i, item) in playlist.iter().enumerate() {
        let id = format!("{:>4}", i + 1);
        let is_current = item.current.unwrap_or(false);
        let cursor = if is_current {
            if pause { "-" } else { "*" }
        } else {
            " "
        };
        let name = control::display_name(&item.filename, full);
        if plain {
            println!("{name}");
        } else if is_current && tty {
            println!("\x1b[2m{id}\x1b[m {cursor} \x1b[32m{name}\x1b[m");
        } else if tty {
            println!("\x1b[2m{id}\x1b[m {cursor} {name}");
        } else {
            println!("{id} {cursor} {name}");
        }
    }
    Ok(())
}

pub fn print_playlist(path: &Path, plain: bool, full: bool) -> Result<(), String> {
    let playlist = read_playlist(path).ok_or("Unable to read playlist")?;
    let tty = std::io::stdout().is_terminal();
    for (i, path) in playlist.iter().enumerate() {
        let path_str = path.to_string_lossy();
        let name = control::display_name(&path_str, full);
        let id = format!("{:>4}", i + 1);
        if plain {
            println!("{name}");
        } else if tty {
            println!("\x1b[2m{id}\x1b[m {name}");
        } else {
            println!("{id} {name}");
        }
    }
    Ok(())
}

pub fn export_playlist(path: &Path, print: bool, force: bool) -> Result<(), String> {
    let playlist = control::get_playlist()?;
    let base = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let base = base.canonicalize().unwrap_or(base);
    let mut xspf = Playlist::default().clone();
    if let Some(filename) = path.file_name() {
        xspf.set_title(filename.to_string_lossy());
    }
    for item in &playlist {
        let filepath = PathBuf::from(&item.filename);
        if let Ok(location) = filepath.strip_prefix(&base) {
            xspf.add_track(
                Track::default()
                    .add_location(location.to_string_lossy())
                    .set_title(control::display_name(&item.filename, false)),
            );
        } else {
            eprintln!(
                "{} is not in {}: skipping",
                filepath.to_string_lossy(),
                base.to_string_lossy()
            );
        }
    }
    if xspf.track_list.is_empty() {
        return Err("output playlist is empty. export aborted.".into());
    }
    let xml = xspf.to_string_pretty("\t");
    if print {
        println!("{xml}");
    } else {
        if path.exists() && !force {
            return Err("output file exists. use --force to overwrite.".into());
        }
        fs::write(path, xml).map_err(|e| format!("write file: {e}"))?;
    }
    Ok(())
}

pub fn push_to_file(target: &Path, files: &[PathBuf]) -> Result<(), String> {
    if files.is_empty() {
        return Ok(());
    }
    let target = crate::config::resolve_tilde(&target.to_string_lossy());
    let dir = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let dir = dir.canonicalize().unwrap_or(dir);
    let mut xspf = if target.exists() {
        Playlist::read_file(&target).map_err(|e| format!("read playlist: {e:?}"))?
    } else {
        let mut playlist = Playlist::default().clone();
        if let Some(filename) = target.file_name() {
            playlist.set_title(filename.to_string_lossy());
        }
        playlist
    };
    for file in files {
        let path = crate::config::resolve_tilde(&file.to_string_lossy());
        let paths = read_playlist(&path).unwrap_or_else(|| vec![path]);
        for filepath in paths {
            let abs = filepath.canonicalize().unwrap_or(filepath.clone());
            if let Ok(location) = abs.strip_prefix(&dir) {
                xspf.add_track(
                    Track::default()
                        .add_location(location.to_string_lossy())
                        .set_title(control::display_name(&abs.to_string_lossy(), false)),
                );
            } else {
                eprintln!(
                    "{} is not in {}: skipping",
                    filepath.to_string_lossy(),
                    dir.to_string_lossy()
                );
            }
        }
    }
    let xml = xspf.to_string_pretty("\t");
    fs::write(&target, xml).map_err(|e| format!("write file: {e}"))?;
    Ok(())
}

pub fn remove_from_file(target: &Path, index: usize) -> Result<(), String> {
    let target = crate::config::resolve_tilde(&target.to_string_lossy());
    let mut xspf = Playlist::read_file(&target).map_err(|e| format!("read playlist: {e:?}"))?;
    if index == 0 || index > xspf.track_list.len() {
        return Err(format!("no track at index {index}"));
    }
    xspf.track_list.remove(index - 1);
    let xml = xspf.to_string_pretty("\t");
    fs::write(&target, xml).map_err(|e| format!("write file: {e}"))?;
    Ok(())
}

pub fn move_in_file(target: &Path, from: usize, to: usize) -> Result<(), String> {
    let target = crate::config::resolve_tilde(&target.to_string_lossy());
    let mut xspf = Playlist::read_file(&target).map_err(|e| format!("read playlist: {e:?}"))?;
    let len = xspf.track_list.len();
    if from == 0 || from > len {
        return Err(format!("no track at index {from}"));
    }
    if to == 0 || to > len {
        return Err(format!("invalid destination index {to}"));
    }
    if from == to {
        return Ok(());
    }
    let item = xspf.track_list.remove(from - 1);
    xspf.track_list.insert(to - 1, item);
    let xml = xspf.to_string_pretty("\t");
    fs::write(&target, xml).map_err(|e| format!("write file: {e}"))?;
    Ok(())
}

pub fn read_playlist(path: &Path) -> Option<Vec<PathBuf>> {
    let dir = path.parent()?;
    let playlist = Playlist::read_file(path).ok()?;
    Some(
        playlist
            .track_list
            .iter()
            .filter_map(|item| item.location.first().map(|location| dir.join(location)))
            .filter(|filepath| filepath.exists())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{FakeServer, Temp, handler_fn, mpv_ok};
    use serde_json::{Value, json};
    use std::path::Path;

    fn mk_xspf(dir: &Path, name: &str, tracks: &[&str]) -> PathBuf {
        let mut pl = Playlist::default().clone();
        for t in tracks {
            pl.add_track(
                Track::default()
                    .add_location(t.to_string())
                    .set_title(t.to_string()),
            );
        }
        let path = dir.join(name);
        fs::write(&path, pl.to_string_pretty("\t")).unwrap();
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
            match cmd.first().and_then(|v| v.as_str()).unwrap_or("") {
                "get_property" => match cmd.get(1).and_then(|v| v.as_str()).unwrap_or("") {
                    "playlist" => mpv_ok(two_track_playlist()),
                    "pause" => mpv_ok(json!(false)),
                    _ => mpv_ok(Value::Null),
                },
                _ => mpv_ok(Value::Null),
            }
        })
    }

    /// Handler that reports tracks inside `base/music/`, so export tests can
    /// verify relative locations without hitting the skip-outside-directory
    /// path.
    fn music_handler(base: &Path) -> crate::test_util::MpvHandler {
        let tracks = json!([
            { "filename": base.join("music/a.mp3").to_string_lossy(), "current": true },
            { "filename": base.join("music/b.mp3").to_string_lossy(), "current": false },
        ]);
        handler_fn(move |req| {
            let cmd = req
                .get("command")
                .and_then(|c| c.as_array())
                .cloned()
                .unwrap_or_default();
            match cmd.first().and_then(|v| v.as_str()).unwrap_or("") {
                "get_property" => match cmd.get(1).and_then(|v| v.as_str()).unwrap_or("") {
                    "playlist" => mpv_ok(tracks.clone()),
                    "pause" => mpv_ok(json!(false)),
                    _ => mpv_ok(Value::Null),
                },
                _ => mpv_ok(Value::Null),
            }
        })
    }

    #[test]
    fn read_playlist_missing_file_is_none() {
        assert!(read_playlist(Path::new("/nonexistent/missing.xspf")).is_none());
    }

    #[test]
    fn read_playlist_returns_existing_tracks() {
        let dir = Temp::new("read");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        std::fs::write(dir.join("music/here.flac"), b"x").unwrap();
        std::fs::write(dir.join("music/gone.flac"), b"x").unwrap();
        let pl = mk_xspf(
            dir.path(),
            "list.xspf",
            &["music/here.flac", "music/nowhere.flac", "music/gone.flac"],
        );
        let files = read_playlist(&pl).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["here.flac", "gone.flac"]);
        std::fs::remove_dir_all(dir.join("music")).unwrap();
    }

    #[test]
    fn print_current_playlist_plain() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            print_current_playlist(true, false).unwrap();
        });
    }

    #[test]
    fn print_current_playlist_decorated() {
        let server = FakeServer::start(default_handler());
        server.with_env(|| {
            print_current_playlist(false, true).unwrap();
        });
    }

    #[test]
    fn print_playlist_plain_and_decorated() {
        let dir = Temp::new("print");
        let pl = mk_xspf(dir.path(), "x.xspf", &["a/track.mp3"]);
        print_playlist(&pl, true, false).unwrap();
        print_playlist(&pl, false, false).unwrap();
    }

    #[test]
    fn print_playlist_unreadable_errors() {
        assert!(
            print_playlist(Path::new("/nonexistent/x.xspf"), true, false)
                .unwrap_err()
                .contains("Unable to read playlist")
        );
    }

    #[test]
    fn export_writes_file_with_relative_locations() {
        let dir = Temp::new("export");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        std::fs::write(dir.join("music/a.mp3"), b"x").unwrap();
        std::fs::write(dir.join("music/b.mp3"), b"x").unwrap();
        let server = FakeServer::start(music_handler(dir.path()));
        let out = dir.join("out.xspf");
        server.with_env(|| {
            export_playlist(&out, false, false).unwrap();
        });
        let pl = Playlist::read_file(&out).unwrap();
        assert_eq!(pl.track_list.len(), 2);
        assert_eq!(pl.track_list[0].location[0], "music/a.mp3");
        assert_eq!(pl.track_list[1].location[0], "music/b.mp3");
    }

    #[test]
    fn export_print_to_stdout() {
        let dir = Temp::new("export-print");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        let server = FakeServer::start(music_handler(dir.path()));
        let out = dir.join("out.xspf");
        server.with_env(|| {
            export_playlist(&out, true, false).unwrap();
        });
    }

    #[test]
    fn export_refuses_to_overwrite_without_force() {
        let dir = Temp::new("export-force");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        let server = FakeServer::start(music_handler(dir.path()));
        let out = dir.join("out.xspf");
        std::fs::write(&out, "existing").unwrap();
        server.with_env(|| {
            let err = export_playlist(&out, false, false).unwrap_err();
            assert!(err.contains("use --force"));
        });
    }

    #[test]
    fn export_force_overwrites() {
        let dir = Temp::new("export-force-ok");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        let server = FakeServer::start(music_handler(dir.path()));
        let out = dir.join("out.xspf");
        std::fs::write(&out, "existing").unwrap();
        server.with_env(|| {
            export_playlist(&out, false, true).unwrap();
        });
    }

    #[test]
    fn export_empty_playlist_aborts() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!([]))));
        let dir = Temp::new("export-empty");
        let out = dir.join("out.xspf");
        server.with_env(|| {
            let err = export_playlist(&out, false, false).unwrap_err();
            assert!(err.contains("output playlist is empty"));
        });
    }

    #[test]
    fn export_skips_files_outside_base() {
        let server = FakeServer::start(handler_fn(|_| {
            mpv_ok(json!([
                { "filename": "/etc/some.conf", "current": false },
            ]))
        }));
        let dir = Temp::new("export-skip");
        let out = dir.join("out.xspf");
        server.with_env(|| {
            let err = export_playlist(&out, false, false).unwrap_err();
            assert!(err.contains("output playlist is empty"));
        });
    }

    #[test]
    fn push_to_file_creates_new_playlist() {
        let dir = Temp::new("push-new");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        std::fs::write(dir.join("music/t.flac"), b"x").unwrap();
        let target = dir.join("out.xspf");
        push_to_file(&target, &[dir.join("music/t.flac")]).unwrap();
        assert!(target.exists());
        let pl = Playlist::read_file(&target).unwrap();
        assert_eq!(pl.track_list.len(), 1);
        assert_eq!(pl.track_list[0].location[0], "music/t.flac");
    }

    #[test]
    fn push_to_file_empty_files_is_noop() {
        let dir = Temp::new("push-empty");
        let target = dir.join("out.xspf");
        push_to_file(&target, &[]).unwrap();
        assert!(!target.exists());
    }

    #[test]
    fn push_to_file_appends_to_existing() {
        let dir = Temp::new("push-append");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        std::fs::write(dir.join("music/a.flac"), b"x").unwrap();
        std::fs::write(dir.join("music/b.flac"), b"x").unwrap();
        let target = mk_xspf(dir.path(), "out.xspf", &["music/a.flac"]);
        push_to_file(&target, &[dir.join("music/b.flac")]).unwrap();
        let pl = Playlist::read_file(&target).unwrap();
        assert_eq!(pl.track_list.len(), 2);
    }

    #[test]
    fn push_to_file_expands_playlist_argument() {
        let dir = Temp::new("push-plarg");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        std::fs::write(dir.join("music/a.flac"), b"x").unwrap();
        std::fs::write(dir.join("music/b.flac"), b"x").unwrap();
        let sub = mk_xspf(dir.path(), "sub.xspf", &["music/a.flac", "music/b.flac"]);
        let target = dir.join("out.xspf");
        push_to_file(&target, &[sub]).unwrap();
        let pl = Playlist::read_file(&target).unwrap();
        assert_eq!(pl.track_list.len(), 2);
    }

    #[test]
    fn push_to_file_skips_outside_dir() {
        let dir = Temp::new("push-skip");
        let target = dir.join("out.xspf");
        push_to_file(&target, &[PathBuf::from("/etc/hostname")]).unwrap();
        let pl = Playlist::read_file(&target).unwrap();
        assert_eq!(pl.track_list.len(), 0);
    }

    #[test]
    fn push_to_file_bad_existing_target_errors() {
        let dir = Temp::new("push-bad");
        let target = dir.join("out.xspf");
        std::fs::write(&target, "not xml").unwrap();
        assert!(push_to_file(&target, &[dir.join("f.mp3")]).is_err());
    }

    #[test]
    fn remove_from_file_removes_track() {
        let dir = Temp::new("rm-ok");
        let target = mk_xspf(dir.path(), "out.xspf", &["a.flac", "b.flac", "c.flac"]);
        remove_from_file(&target, 2).unwrap();
        let pl = Playlist::read_file(&target).unwrap();
        assert_eq!(pl.track_list.len(), 2);
        assert_eq!(pl.track_list[0].location[0], "a.flac");
        assert_eq!(pl.track_list[1].location[0], "c.flac");
    }

    #[test]
    fn remove_from_file_invalid_index() {
        let dir = Temp::new("rm-idx");
        let target = mk_xspf(dir.path(), "out.xspf", &["a.flac"]);
        assert!(remove_from_file(&target, 0).is_err());
        assert!(remove_from_file(&target, 5).is_err());
    }

    #[test]
    fn remove_from_file_missing_target_errors() {
        assert!(remove_from_file(Path::new("/nonexistent/x.xspf"), 1).is_err());
    }

    #[test]
    fn move_in_file_reorders() {
        let dir = Temp::new("mv-ok");
        let target = mk_xspf(dir.path(), "out.xspf", &["a.flac", "b.flac", "c.flac"]);
        move_in_file(&target, 1, 3).unwrap();
        let pl = Playlist::read_file(&target).unwrap();
        let locs: Vec<String> = pl
            .track_list
            .iter()
            .map(|t| t.location[0].clone())
            .collect();
        assert_eq!(locs, vec!["b.flac", "c.flac", "a.flac"]);
    }

    #[test]
    fn move_in_file_invalid_indexes() {
        let dir = Temp::new("mv-idx");
        let target = mk_xspf(dir.path(), "out.xspf", &["a.flac", "b.flac"]);
        assert!(move_in_file(&target, 0, 1).is_err());
        assert!(move_in_file(&target, 1, 0).is_err());
        assert!(move_in_file(&target, 3, 1).is_err());
    }

    #[test]
    fn move_in_file_same_index_is_noop() {
        let dir = Temp::new("mv-same");
        let target = mk_xspf(dir.path(), "out.xspf", &["a.flac", "b.flac"]);
        move_in_file(&target, 2, 2).unwrap();
        let pl = Playlist::read_file(&target).unwrap();
        assert_eq!(pl.track_list.len(), 2);
    }

    #[test]
    fn move_in_file_missing_target_errors() {
        assert!(move_in_file(Path::new("/nonexistent/x.xspf"), 1, 2).is_err());
    }

    #[test]
    fn export_uses_display_name_for_titles() {
        let dir = Temp::new("export-title");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        let server = FakeServer::start(music_handler(dir.path()));
        let out = dir.join("out.xspf");
        server.with_env(|| {
            export_playlist(&out, false, false).unwrap();
        });
        let pl = Playlist::read_file(&out).unwrap();
        assert_eq!(pl.track_list[0].title.as_deref(), Some("a.mp3"));
    }

    #[test]
    fn display_name_used_for_pushes() {
        let dir = Temp::new("display");
        std::fs::create_dir_all(dir.join("music")).unwrap();
        let f = dir.join("music/x.flac");
        std::fs::write(&f, b"x").unwrap();
        let target = dir.join("out.xspf");
        push_to_file(&target, &[f]).unwrap();
        let pl = Playlist::read_file(&target).unwrap();
        assert_eq!(pl.track_list[0].title.as_deref(), Some("x.flac"));
    }
}
