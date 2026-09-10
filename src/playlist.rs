use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
};
use xspf::Playlist;

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
