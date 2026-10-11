use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{self, Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::config;

const STATUS_OK: u8 = b'1';
const STATUS_ERR: u8 = b'0';

/// mpv's pid once spawned, so the signal handler can relay signals to it
static CHILD_PID: AtomicU32 = AtomicU32::new(0);
/// Termination signal caught before mpv existed yet, 0 when none
static PENDING_SIG: AtomicI32 = AtomicI32::new(0);

extern "C" fn on_termination(sig: libc::c_int) {
    PENDING_SIG.store(sig, Ordering::SeqCst);
    let pid = CHILD_PID.load(Ordering::SeqCst);
    if pid != 0 {
        // Relay to mpv; the supervisor then runs the usual cleanup
        unsafe { libc::kill(pid as i32, sig) };
    }
}

pub fn get_pid() -> Option<u32> {
    let content = fs::read_to_string(config::mpvd_pid()).ok()?;
    let pid: u32 = content.trim().parse().ok()?;
    if !process_alive(pid) {
        // Self-heal: the trap cannot run when the supervisor itself died,
        // so any command noticing a dead daemon clears the leftovers
        let _ = fs::remove_file(config::mpvd_sock());
        let _ = fs::remove_file(config::mpvd_pid());
        return None;
    }
    Some(pid)
}

fn process_alive(pid: u32) -> bool {
    let proc_dir = Path::new("/proc").join(pid.to_string());
    proc_dir.exists() && !is_zombie(pid)
}

fn is_zombie(pid: u32) -> bool {
    let Ok(stat) = fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("stat")) else {
        return false;
    };
    // The state character follows the comm field, which may contain parens
    match stat.rfind(')') {
        Some(i) => stat[i + 2..].starts_with('Z'),
        None => false,
    }
}

/// Start mpv daemon. Returns exit code 2 if daemon is already running.
pub fn start() -> ExitCode {
    if get_pid().is_some() {
        return ExitCode::from(2);
    }
    let sock = config::mpvd_sock();
    let pid_path = config::mpvd_pid();

    let Ok((mut parent_stream, mut child_stream)) = UnixStream::pair() else {
        eprintln!("failed to create handshake socket");
        return ExitCode::from(1);
    };

    match unsafe { libc::fork() } {
        -1 => {
            eprintln!("failed to fork: {}", std::io::Error::last_os_error());
            ExitCode::from(1)
        }
        0 => {
            drop(parent_stream);
            supervise(&sock, &pid_path, &mut child_stream)
        }
        _ => {
            drop(child_stream);
            await_supervisor(&mut parent_stream)
        }
    }
}

/// Child side of the trap: becomes session leader, spawns mpv as a direct
/// child, then blocks until mpv dies for any reason before cleaning up.
fn supervise(sock: &Path, pid_path: &Path, status: &mut UnixStream) -> ! {
    // Daemonize: create a new session and detach from the controlling terminal.
    unsafe { libc::setsid() };
    install_signal_handlers();

    let Ok(mut child) = Command::new("mpv")
        .args([
            "--idle",
            "--no-video",
            &format!("--input-ipc-server={}", sock.display()),
        ])
        .current_dir(home_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        fail(status, "failed to spawn mpv");
    };

    let pid = child.id();
    CHILD_PID.store(pid, Ordering::SeqCst);
    // A signal may have slipped in between fork and spawn
    let pending = PENDING_SIG.swap(0, Ordering::SeqCst);
    if pending != 0 {
        unsafe { libc::kill(pid as i32, pending) };
    }
    if let Err(e) = fs::write(pid_path, format!("{pid}\n")) {
        let _ = child.kill();
        fail(status, &format!("failed to write pid file: {e}"));
    }

    // mpv creates the IPC socket asynchronously after spawn, and the
    // handshake must only report success once the daemon is usable
    if !await_socket(sock, Duration::from_secs(10)) {
        let _ = child.kill();
        fail(status, "mpv failed to create IPC socket");
    }

    write_status(status, STATUS_OK, "");
    let _ = status.shutdown(Shutdown::Write);
    let _ = child.wait();

    // Clean up socket and pid files once mpv is gone
    let _ = fs::remove_file(sock);
    let _ = fs::remove_file(pid_path);
    process::exit(0)
}

/// Probe readiness with real connect attempts rather than socket file
/// existence: there is a window between bind and listen where connecting
/// to an existing inode is still refused
fn await_socket(sock: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(stream) = UnixStream::connect(sock) {
            drop(stream);
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(25));
    }
}

/// Parent side of the handshake: reads the supervisor's status report without
/// reaping it, leaving the long-lived supervisor to be reparented on exit.
fn await_supervisor(stream: &mut UnixStream) -> ExitCode {
    let status = read_status(stream);
    match status.first() {
        Some(&STATUS_OK) => ExitCode::SUCCESS,
        Some(&STATUS_ERR) => {
            let msg = String::from_utf8_lossy(&status[1..]);
            if msg.is_empty() {
                eprintln!("failed to start mpv daemon");
            } else {
                eprintln!("{msg}");
            }
            ExitCode::from(1)
        }
        _ => {
            eprintln!("failed to start mpv daemon");
            ExitCode::from(1)
        }
    }
}

fn fail(status: &mut UnixStream, msg: &str) -> ! {
    write_status(status, STATUS_ERR, msg);
    process::exit(1)
}

/// Trap termination signals by relaying them to mpv, so `pkill mpvd` and
/// friends tear the whole daemon down through the regular death-and-cleanup
/// path instead of orphaning it
fn install_signal_handlers() {
    let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
    sa.sa_sigaction = on_termination as extern "C" fn(libc::c_int) as usize;
    unsafe { libc::sigemptyset(&mut sa.sa_mask) };
    sa.sa_flags = libc::SA_RESTART;
    for sig in [libc::SIGHUP, libc::SIGINT, libc::SIGTERM] {
        unsafe { libc::sigaction(sig, &sa, std::ptr::null_mut()) };
    }
}

fn write_status(writer: &mut impl Write, kind: u8, msg: &str) {
    let mut buf = Vec::with_capacity(1 + msg.len());
    buf.push(kind);
    buf.extend_from_slice(msg.as_bytes());
    write_all(writer, &buf);
}

fn write_all(writer: &mut impl Write, data: &[u8]) {
    let _ = writer.write_all(data);
}

fn read_status(reader: &mut impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = reader.read_to_end(&mut buf);
    buf
}

pub fn kill() -> ExitCode {
    let mut success = false;
    if let Some(pid) = get_pid()
        && unsafe { libc::kill(pid as i32, libc::SIGTERM) } == 0
    {
        success = true;
    }
    // Clean up socket and pid files
    let _ = fs::remove_file(config::mpvd_sock());
    let _ = fs::remove_file(config::mpvd_pid());
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

pub fn pid() -> ExitCode {
    match get_pid() {
        Some(pid) => {
            println!("{pid}");
            ExitCode::SUCCESS
        }
        None => ExitCode::from(1),
    }
}

pub fn env() {
    println!("MPVD_SOCK={}", config::mpvd_sock().display());
    println!("MPVD_PID={}", config::mpvd_pid().display());
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{Temp, with_env};
    use std::os::unix::net::UnixListener;

    fn daemon_env(dir: &Temp) -> Vec<(String, Option<String>)> {
        vec![
            (
                "MPVD_SOCK".into(),
                Some(dir.sock().to_string_lossy().into()),
            ),
            ("MPVD_PID".into(), Some(dir.pid().to_string_lossy().into())),
        ]
    }

    fn with_daemon_env(dir: &Temp, f: impl FnOnce()) {
        let envs = daemon_env(dir);
        let refs: Vec<(&str, Option<&str>)> = envs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_deref()))
            .collect();
        with_env(&refs, f);
    }

    #[test]
    fn get_pid_none_without_files() {
        let dir = Temp::new("daemon");
        with_daemon_env(&dir, || assert_eq!(get_pid(), None));
    }

    #[test]
    fn get_pid_detects_current_process() {
        let dir = Temp::new("daemon");
        fs::write(dir.pid(), format!("{}\n", std::process::id())).unwrap();
        with_daemon_env(&dir, || {
            assert_eq!(get_pid(), Some(std::process::id()));
        });
    }

    #[test]
    fn get_pid_unparseable_returns_none() {
        let dir = Temp::new("daemon");
        fs::write(dir.pid(), "not-a-pid").unwrap();
        with_daemon_env(&dir, || assert_eq!(get_pid(), None));
    }

    #[test]
    fn get_pid_dead_pid_self_heals() {
        let dir = Temp::new("daemon");
        fs::write(dir.pid(), "999999999\n").unwrap();
        fs::write(dir.sock(), b"").unwrap();
        with_daemon_env(&dir, || {
            assert_eq!(get_pid(), None);
            assert!(!dir.sock().exists());
            assert!(!dir.pid().exists());
        });
    }

    #[test]
    fn process_alive_reflects_system_state() {
        assert!(process_alive(std::process::id()));
        assert!(!process_alive(999_999_999));
    }

    #[test]
    fn is_zombie_is_false_for_self() {
        assert!(!is_zombie(std::process::id()));
        assert!(!is_zombie(999_999_999));
    }

    #[test]
    fn await_socket_succeeds_when_bound() {
        let dir = Temp::new("daemon");
        let _listener = UnixListener::bind(dir.sock()).unwrap();
        assert!(await_socket(&dir.sock(), Duration::from_secs(1)));
    }

    #[test]
    fn await_socket_times_out() {
        let dir = Temp::new("daemon");
        assert!(!await_socket(&dir.sock(), Duration::from_millis(20)));
    }

    #[test]
    fn start_returns_2_when_already_running() {
        let dir = Temp::new("daemon");
        fs::write(dir.pid(), format!("{}\n", std::process::id())).unwrap();
        with_daemon_env(&dir, || {
            assert_eq!(start(), ExitCode::from(2));
        });
    }

    #[test]
    fn kill_without_daemon_cleans_up_and_errors() {
        let dir = Temp::new("daemon");
        with_daemon_env(&dir, || {
            assert_eq!(kill(), ExitCode::from(1));
        });
    }

    #[test]
    fn pid_prints_and_fails_without_daemon() {
        let dir = Temp::new("daemon");
        with_daemon_env(&dir, || {
            assert_eq!(pid(), ExitCode::from(1));
            fs::write(dir.pid(), format!("{}\n", std::process::id())).unwrap();
            assert_eq!(pid(), ExitCode::SUCCESS);
        });
    }

    #[test]
    fn env_prints_mpvd_paths() {
        let dir = Temp::new("daemon");
        let s = dir.sock().to_string_lossy().to_string();
        with_env(
            &[
                ("MPVD_SOCK", Some(s.as_str())),
                ("XDG_RUNTIME_DIR", None),
                ("HOME", None),
            ],
            env,
        );
    }

    #[test]
    fn home_dir_fallback() {
        with_env(&[("HOME", Some("/x"))], || {
            assert_eq!(home_dir(), PathBuf::from("/x"))
        });
        with_env(&[("HOME", None)], || {
            assert_eq!(home_dir(), PathBuf::from("."))
        });
    }

    #[test]
    fn write_read_status_roundtrip() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        write_status(&mut writer, STATUS_OK, "hello");
        write_status(&mut writer, STATUS_ERR, "boom");
        writer.shutdown(Shutdown::Write).unwrap();
        let out = read_status(&mut reader);
        assert_eq!(out, b"1hello0boom".to_vec());
    }

    #[test]
    fn write_status_empty_and_read_status_closed() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        write_status(&mut writer, STATUS_OK, "");
        writer.shutdown(Shutdown::Write).unwrap();
        let out = read_status(&mut reader);
        assert_eq!(out, b"1".to_vec());
    }

    #[test]
    fn write_all_writes_everything() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        let payload = vec![b'x'; 4096];
        write_all(&mut writer, &payload);
        writer.shutdown(Shutdown::Write).unwrap();
        let out = read_status(&mut reader);
        assert_eq!(out, payload);
    }
}
