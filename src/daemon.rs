use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::config;

static CHILD_PID: AtomicU32 = AtomicU32::new(0);

pub fn get_pid() -> Option<u32> {
    let pid = std::fs::read_to_string(config::get_pidfile_path())
        .ok()?
        .trim()
        .parse()
        .ok()?;
    match get_process_state(pid) {
        ProcessState::Alive | ProcessState::Unknown => Some(pid),
        ProcessState::Dead => {
            cleanup();
            None
        }
    }
}

pub fn start() -> Result<(), String> {
    if get_pid().is_some() {
        return Err("mpv is already running".into());
    }
    let Ok((mut parent_stream, mut child_stream)) = UnixStream::pair() else {
        return Err("failed to create handshake socket".into());
    };
    match unsafe { libc::fork() } {
        -1 => Err("failed to fork".into()),
        0 => supervise(&mut child_stream),
        _ => await_supervisor(&mut parent_stream),
    }
}

pub fn kill() -> Result<(), String> {
    match get_pid() {
        None => Err("mpvd is not running".into()),
        Some(pid) => {
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            Ok(())
        }
    }
}

fn supervise(status: &mut UnixStream) -> ! {
    unsafe { libc::setsid() };
    if let Some(home) = std::env::var_os("HOME") {
        let _ = std::env::set_current_dir(home);
    }
    let Ok(mut child) = Command::new("mpv")
        .args([
            "--idle",
            "--no-video",
            &format!("--input-ipc-server={}", config::get_socket_path().display()),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        let _ = status.write_all(b"failed to spawn mpv");
        let _ = status.shutdown(std::net::Shutdown::Write);
        std::process::exit(1)
    };
    CHILD_PID.store(child.id(), Ordering::SeqCst);
    bind_signal_handlers();
    let Ok(_) = std::fs::write(
        config::get_pidfile_path(),
        format!("{}\n", std::process::id()),
    ) else {
        let _ = child.kill();
        let _ = status.write_all(b"failed to write PID file");
        let _ = status.shutdown(std::net::Shutdown::Write);
        std::process::exit(1)
    };
    if !await_socket(Duration::from_secs(5)) {
        let _ = child.kill();
        let _ = status.write_all(b"mpv socket timed out");
        let _ = status.shutdown(std::net::Shutdown::Write);
        std::process::exit(1)
    }
    let _ = status.write_all(b"OK");
    let _ = status.shutdown(std::net::Shutdown::Write);
    let _ = child.wait();
    cleanup();
    std::process::exit(0)
}

fn await_supervisor(status: &mut UnixStream) -> Result<(), String> {
    let mut buf = Vec::new();
    let _ = status.read_to_end(&mut buf);
    let msg = String::from_utf8_lossy(&buf);
    if msg == "OK" {
        Ok(())
    } else {
        Err(format!("{}", msg))
    }
}

fn bind_signal_handlers() {
    let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
    sa.sa_sigaction = on_termination as extern "C" fn(libc::c_int) as usize;
    unsafe { libc::sigemptyset(&mut sa.sa_mask) };
    sa.sa_flags = libc::SA_RESTART;
    for sig in [libc::SIGHUP, libc::SIGINT, libc::SIGTERM] {
        unsafe { libc::sigaction(sig, &sa, std::ptr::null_mut()) };
    }
}

extern "C" fn on_termination(sig: libc::c_int) {
    let pid = CHILD_PID.load(Ordering::SeqCst);
    if pid != 0 {
        unsafe { libc::kill(pid as i32, sig) };
    }
    cleanup();
    std::process::exit(1)
}

fn await_socket(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if let Ok(_) = UnixStream::connect(config::get_socket_path()) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn cleanup() {
    let _ = std::fs::remove_file(config::get_pidfile_path());
    let _ = std::fs::remove_file(config::get_socket_path());
}

enum ProcessState {
    Alive,
    Dead,
    Unknown,
}

fn get_process_state(pid: u32) -> ProcessState {
    if unsafe { libc::kill(pid as i32, 0) } == 0 {
        ProcessState::Alive
    } else {
        let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
        if errno == libc::ESRCH {
            ProcessState::Dead
        } else {
            ProcessState::Unknown
        }
    }
}
