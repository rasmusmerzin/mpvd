use std::io::IsTerminal;

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
