use serde::Deserialize;
use serde_json::json;

use crate::ipc;

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
    let path = crate::config::resolve_tilde(file);
    ipc::send(&[
        json!("loadfile"),
        json!(path.to_string_lossy()),
        json!("append-play"),
    ])?;
    Ok(())
}

pub fn set_pause(paused: bool) -> Result<(), String> {
    ipc::send(&[json!("set_property"), json!("pause"), json!(paused)])?;
    Ok(())
}

pub fn play_at_index(index: usize) -> Result<(), String> {
    ipc::send(&[json!("playlist-play-index"), json!(index)])?;
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

pub fn insert_next(file: &str) -> Result<(), String> {
    let path = crate::config::resolve_tilde(file);
    ipc::send(&[
        json!("loadfile"),
        json!(path.to_string_lossy()),
        json!("insert-next"),
    ])?;
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
