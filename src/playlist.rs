use std::{
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
};
use xspf::{Playlist, Track};

use crate::control;

pub fn print_playlist(plain: bool, full: bool) -> Result<(), String> {
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
    if xspf.track_list.len() == 0 {
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

pub fn read_playlist(path: &Path) -> Option<Vec<PathBuf>> {
    let dir = path.parent()?;
    if let Some(ext) = &path.extension()
        && ext.to_string_lossy() == "xspf"
        && let Ok(playlist) = Playlist::read_file(&path)
    {
        Some(
            playlist
                .track_list
                .iter()
                .filter_map(|item| item.location.first().map(|location| dir.join(location)))
                .filter(|filepath| filepath.exists())
                .collect(),
        )
    } else {
        None
    }
}
