use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use crate::config;

pub fn get_pid() -> Option<u32> {
    let content = fs::read_to_string(config::mpvd_pid()).ok()?;
    content.trim().parse().ok()
}

pub fn start() -> ExitCode {
    if get_pid().is_some() {
        eprintln!("mpv daemon is already running");
        return ExitCode::from(1);
    }
    let sock = config::mpvd_sock();
    let pid_path = config::mpvd_pid();
    let child = unsafe {
        Command::new("mpv")
            .args([
                "--idle",
                "--no-video",
                &format!("--input-ipc-server={}", sock.display()),
            ])
            .current_dir(dirs())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .pre_exec(|| {
                // Daemonize: create a new session and detach from terminal
                libc::setsid();
                Ok(())
            })
            .spawn()
    };
    match child {
        Ok(proc) => {
            let pid = proc.id();
            if let Err(e) = fs::write(&pid_path, format!("{pid}\n")) {
                eprintln!("failed to write pid file: {e}");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("failed to spawn mpv: {e}");
            ExitCode::from(1)
        }
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

fn dirs() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
