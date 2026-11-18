use std::path::PathBuf;

pub fn get_socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("MPVD_SOCK") {
        resolve_tilde(&path.to_string_lossy())
    } else if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR")
        .or_else(|| std::env::var_os("TMPDIR"))
        .or_else(|| std::env::var_os("HOME"))
    {
        PathBuf::from(dir).join("mpvd.sock")
    } else {
        PathBuf::from("mpvd.sock")
    }
}

pub fn get_pidfile_path() -> PathBuf {
    get_socket_path().parent().unwrap().join("mpvd.pid")
}

pub fn resolve_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~')
        && let Some(home) = std::env::var_os("HOME")
    {
        PathBuf::from(home).join(rest.trim_start_matches('/'))
    } else if !path.starts_with("/")
        && let Ok(pwd) = std::env::current_dir()
    {
        pwd.join(path)
    } else {
        PathBuf::from(path)
    }
}
