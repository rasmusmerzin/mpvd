use std::fs;
use std::path::{Path, PathBuf};

const MPV_AUDIO_EXTS: &[&str] = &[
    "aac", "ac3", "aiff", "ape", "au", "dts", "eac3", "flac", "m4a", "mka", "mp3", "oga", "ogg",
    "ogm", "opus", "thd", "wav", "wma", "wv", "tta",
];

pub fn is_mpv_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| MPV_AUDIO_EXTS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn find_files(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(find_files(&path));
            } else if path.is_file() && is_mpv_audio(&path) {
                results.push(path);
            }
        }
    }
    results
}
