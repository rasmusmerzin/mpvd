use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;
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
    let mut seen = HashSet::new();
    if let Ok(md) = fs::metadata(dir) {
        seen.insert((md.dev(), md.ino()));
    }
    walk(dir, &mut seen)
}

fn walk(dir: &Path, seen: &mut HashSet<(u64, u64)>) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.starts_with('.'))
            {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                let Ok(md) = fs::metadata(&path) else {
                    continue;
                };
                if seen.insert((md.dev(), md.ino())) {
                    results.extend(walk(&path, seen));
                }
            } else if path.is_file() && is_mpv_audio(&path) {
                results.push(path);
            }
        }
    }
    results
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyclic_symlink_terminates() {
        let base = std::env::temp_dir().join(format!("mpvd-find-test-{}", std::process::id()));
        let sub = base.join("a");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("song.flac"), b"x").unwrap();
        std::os::unix::fs::symlink("..", sub.join("up")).unwrap();

        let files = find_files(&base);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("song.flac"));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn linked_dir_visited_once() {
        let base = std::env::temp_dir().join(format!("mpvd-find-test-2-{}", std::process::id()));
        let a = base.join("a");
        fs::create_dir_all(&a).unwrap();
        fs::write(a.join("one.mp3"), b"x").unwrap();
        std::os::unix::fs::symlink(&a, base.join("to-a")).unwrap();

        let files = find_files(&base);
        assert_eq!(files.len(), 1);

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn hidden_entries_ignored() {
        let base = std::env::temp_dir().join(format!("mpvd-find-test-3-{}", std::process::id()));
        fs::create_dir_all(base.join(".hidden")).unwrap();
        fs::write(base.join(".hidden/tune.ogg"), b"x").unwrap();
        fs::write(base.join(".secret.mp3"), b"x").unwrap();
        fs::write(base.join("real.wav"), b"x").unwrap();

        let files = find_files(&base);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("real.wav"));

        fs::remove_dir_all(&base).unwrap();
    }
}
