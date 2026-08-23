use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::config;

const STATUS_OK: u8 = b'1';
const STATUS_ERR: u8 = b'0';

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
    match unsafe { libc::kill(pid as i32, 0) } {
        // A freshly killed child lingers as a zombie until the supervisor
        // reaps it, and zombies answer kill(0) like live processes
        0 => !is_zombie(pid),
        // Process exists but is owned by another user
        -1 => std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM),
        _ => false,
    }
}

fn is_zombie(pid: u32) -> bool {
    let Ok(stat) = fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("stat"))
    else {
        return false;
    };
    // The state character follows the comm field, which may contain parens
    match stat.rfind(')') {
        Some(i) => stat[i + 2..].starts_with('Z'),
        None => false,
    }
}

pub fn start() -> ExitCode {
    if get_pid().is_some() {
        eprintln!("mpv daemon is already running");
        return ExitCode::from(1);
    }
    let sock = config::mpvd_sock();
    let pid_path = config::mpvd_pid();

    let mut fds = [0 as libc::c_int; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        eprintln!(
            "failed to create pipe: {}",
            std::io::Error::last_os_error()
        );
        return ExitCode::from(1);
    }
    // Keep the handshake fds out of mpv's inherited descriptor table so the
    // parent sees EOF once the supervisor reports
    for fd in fds {
        unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
    }

    match unsafe { libc::fork() } {
        -1 => {
            eprintln!("failed to fork: {}", std::io::Error::last_os_error());
            unsafe {
                libc::close(fds[0]);
                libc::close(fds[1]);
            }
            ExitCode::from(1)
        }
        0 => supervise(&sock, &pid_path, fds),
        _ => await_supervisor(fds),
    }
}

/// Child side of the trap: becomes session leader, spawns mpv as a direct
/// child, then blocks until mpv dies for any reason before cleaning up.
fn supervise(sock: &Path, pid_path: &Path, fds: [libc::c_int; 2]) -> ! {
    let (rfd, wfd) = (fds[0], fds[1]);
    unsafe { libc::close(rfd) };
    unsafe {
        // Daemonize: create a new session and detach from terminal
        libc::setsid();
    }
    ignore_signals();
    detach_stdio();

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
        fail(wfd, "failed to spawn mpv");
    };

    let pid = child.id();
    if let Err(e) = fs::write(pid_path, format!("{pid}\n")) {
        let _ = child.kill();
        fail(wfd, &format!("failed to write pid file: {e}"));
    }

    write_status(wfd, STATUS_OK, "");
    unsafe { libc::close(wfd) };
    wait_child(pid as i32);

    // Clean up socket and pid files once mpv is gone
    let _ = fs::remove_file(sock);
    let _ = fs::remove_file(pid_path);
    unsafe { libc::_exit(0) }
}

/// Parent side of the handshake: reads the supervisor's status report without
/// reaping it, leaving the long-lived supervisor to be reparented on exit.
fn await_supervisor(fds: [libc::c_int; 2]) -> ExitCode {
    let (rfd, wfd) = (fds[0], fds[1]);
    unsafe { libc::close(wfd) };
    let status = read_status(rfd);
    unsafe { libc::close(rfd) };
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

fn fail(wfd: libc::c_int, msg: &str) -> ! {
    write_status(wfd, STATUS_ERR, msg);
    unsafe { libc::close(wfd) };
    unsafe { libc::_exit(1) }
}

/// Keep the trap alive: stray terminal signals must not kill the supervisor
/// before it gets to clean up after mpv
fn ignore_signals() {
    let mut sa: libc::sigaction = unsafe { std::mem::zeroed() };
    sa.sa_sigaction = libc::SIG_IGN;
    unsafe { libc::sigemptyset(&mut sa.sa_mask) };
    for sig in [libc::SIGHUP, libc::SIGINT, libc::SIGTERM] {
        unsafe { libc::sigaction(sig, &sa, std::ptr::null_mut()) };
    }
}

fn detach_stdio() {
    let devnull = b"/dev/null\0";
    let fd =
        unsafe { libc::open(devnull.as_ptr().cast(), libc::O_RDWR) };
    if fd < 0 {
        return;
    }
    for target in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        unsafe { libc::dup2(fd, target) };
    }
    if fd > libc::STDERR_FILENO {
        unsafe { libc::close(fd) };
    }
}

fn write_status(fd: libc::c_int, kind: u8, msg: &str) {
    let mut buf = Vec::with_capacity(1 + msg.len());
    buf.push(kind);
    buf.extend_from_slice(msg.as_bytes());
    write_all(fd, &buf);
}

fn write_all(fd: libc::c_int, mut data: &[u8]) {
    while !data.is_empty() {
        let written = unsafe { libc::write(fd, data.as_ptr().cast(), data.len()) };
        if written < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return;
        }
        data = &data[written as usize..];
    }
}

fn read_status(fd: libc::c_int) -> Vec<u8> {
    let mut chunk = [0u8; 512];
    let mut buf = Vec::new();
    loop {
        let n = unsafe { libc::read(fd, chunk.as_mut_ptr().cast(), chunk.len()) };
        if n < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            break;
        }
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n as usize]);
    }
    buf
}

fn wait_child(pid: libc::pid_t) {
    loop {
        let mut status = 0;
        let reaped = unsafe { libc::waitpid(pid, &mut status, 0) };
        if reaped < 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
        {
            continue;
        }
        break;
    }
}

pub fn kill() -> ExitCode {
    let mut success = true;
    if let Some(pid) = get_pid() {
        // Send SIGTERM
        unsafe {
            if libc::kill(pid as i32, libc::SIGTERM) != 0 {
                success = false;
            }
        }
    } else {
        success = false;
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
