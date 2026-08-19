use std::env;
use std::path::PathBuf;

fn resolve_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~')
        && let Some(home) = env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(path)
}

pub fn mpvd_sock() -> PathBuf {
    if let Ok(val) = env::var("MPVD_SOCK") {
        return resolve_tilde(&val);
    }
    let base = env::var("XDG_RUNTIME_DIR")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_else(|_| "~".into());
    resolve_tilde(&format!("{base}/mpvd.sock"))
}

pub fn mpvd_pid() -> PathBuf {
    if let Ok(val) = env::var("MPVD_PID") {
        return resolve_tilde(&val);
    }
    let sock = mpvd_sock();
    let parent = sock.parent().unwrap_or(std::path::Path::new("."));
    parent.join("mpvd.pid")
}
