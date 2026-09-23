use std::env;
use std::path::PathBuf;

pub const DEFAULT_MUSIC_DIR: &str = "~/Music";

pub fn resolve_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~')
        && let Some(home) = env::var_os("HOME")
    {
        PathBuf::from(home).join(rest.trim_start_matches('/'))
    } else if !path.starts_with("/")
        && let Ok(pwd) = env::current_dir()
    {
        pwd.join(path)
    } else {
        PathBuf::from(path)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::with_env;

    #[test]
    fn resolve_tilde_expands_home() {
        with_env(&[("HOME", Some("/home/tester"))], || {
            assert_eq!(
                resolve_tilde("~/Music"),
                PathBuf::from("/home/tester/Music")
            );
            assert_eq!(resolve_tilde("~"), PathBuf::from("/home/tester"));
            assert_eq!(
                resolve_tilde("~/a/~/b"),
                PathBuf::from("/home/tester/a/~/b")
            );
        });
    }

    #[test]
    fn resolve_tilde_tilde_without_home_uses_cwd() {
        with_env(
            &[("HOME", None), ("MPVD_SOCK", None), ("MPVD_PID", None)],
            || {
                let cwd = std::env::current_dir().unwrap();
                assert_eq!(resolve_tilde("rel/x"), cwd.join("rel/x"));
            },
        );
    }

    #[test]
    fn resolve_tilde_absolute_path_unchanged() {
        with_env(&[("HOME", Some("/home/tester"))], || {
            assert_eq!(resolve_tilde("/abs/path"), PathBuf::from("/abs/path"));
        });
    }

    #[test]
    fn mpvd_sock_prefers_env_var() {
        with_env(
            &[
                ("MPVD_SOCK", Some("~/my.sock")),
                ("HOME", Some("/home/tester")),
            ],
            || assert_eq!(mpvd_sock(), PathBuf::from("/home/tester/my.sock")),
        );
    }

    #[test]
    fn mpvd_sock_falls_back_to_xdg_runtime() {
        with_env(
            &[
                ("MPVD_SOCK", None),
                ("XDG_RUNTIME_DIR", Some("/run/user/1000")),
            ],
            || assert_eq!(mpvd_sock(), PathBuf::from("/run/user/1000/mpvd.sock")),
        );
    }

    #[test]
    fn mpvd_sock_falls_back_to_home() {
        with_env(
            &[
                ("MPVD_SOCK", None),
                ("XDG_RUNTIME_DIR", None),
                ("HOME", Some("/home/tester")),
            ],
            || assert_eq!(mpvd_sock(), PathBuf::from("/home/tester/mpvd.sock")),
        );
    }

    #[test]
    fn mpvd_pid_prefers_env_var() {
        with_env(&[("MPVD_PID", Some("/var/run/mpvd.pid"))], || {
            assert_eq!(mpvd_pid(), PathBuf::from("/var/run/mpvd.pid"))
        });
    }

    #[test]
    fn mpvd_pid_derives_from_sock() {
        with_env(
            &[("MPVD_PID", None), ("MPVD_SOCK", Some("/tmp/x/sock"))],
            || assert_eq!(mpvd_pid(), PathBuf::from("/tmp/x/mpvd.pid")),
        );
    }
}
