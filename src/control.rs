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
