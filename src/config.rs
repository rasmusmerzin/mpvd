use std::path::PathBuf;

pub fn get_socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("MPVD_SOCK") {
        PathBuf::from(path)
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
