use serde::Deserialize;
use serde_json::json;

use crate::config;
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
    let path = config::resolve_tilde(file);
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
    let path = config::resolve_tilde(file);
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
    let _ = set_pause(false);
    Ok(())
}

pub fn go_next() -> Result<(), String> {
    ipc::send(&[json!("playlist-next")])?;
    Ok(())
}

pub fn go_previous() -> Result<(), String> {
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

pub fn clear_playlist() -> Result<(), String> {
    ipc::send(&[json!("playlist-clear")])?;
    let _ = ipc::send(&[json!("playlist-remove"), json!(0)]);
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
    let pos = format_time(time);
    let dur = format_time(duration);
    format!("{pos}/{dur}")
}

pub fn format_time(time: f64) -> String {
    let secs = time as i64;
    let mm = format!("{:02}", secs / 60);
    let ss = format!("{:02}", secs % 60);
    format!("{mm}:{ss}")
}

pub fn seek(seconds: f64) -> Result<(), String> {
    ipc::send(&[json!("seek"), json!(seconds), json!("relative")])?;
    Ok(())
}

pub fn get_current() -> Result<String, String> {
    get_playlist()?
        .iter()
        .find(|item| item.current == Some(true))
        .map(|item| item.filename.clone())
        .ok_or("no current track".into())
}

pub fn display_name(filepath: &str, absolute: bool) -> &str {
    if absolute {
        filepath
    } else {
        filepath.rsplit('/').next().unwrap_or(filepath)
    }
}
