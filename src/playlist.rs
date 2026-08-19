use crate::control;

pub fn print_playlist(plain: bool, full: bool) -> Result<String, String> {
    let pause = control::get_pause()?;
    let playlist = control::get_playlist()?;
    let lines: Vec<String> = playlist
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let id = format!("{:>4}", i + 1);
            let is_current = item.current.unwrap_or(false);
            let cursor = if is_current {
                if pause { "-" } else { "*" }
            } else {
                " "
            };
            let name = if full {
                item.filename.clone()
            } else {
                item.filename
                    .rsplit('/')
                    .next()
                    .unwrap_or(&item.filename)
                    .to_string()
            };
            if plain {
                name
            } else {
                let mut line = format!("\x1b[2m{id}\x1b[m {cursor} ");
                line.push_str(
                    &(if is_current {
                        format!("\x1b[32m{name}\x1b[m")
                    } else {
                        name
                    }),
                );
                line
            }
        })
        .collect();
    Ok(lines.join("\n"))
}
