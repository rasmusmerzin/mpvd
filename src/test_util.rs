#![allow(dead_code)]

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

/// How a fake mpv server should answer one request.
pub enum Reply {
    /// Write a response and keep reading the connection.
    Respond(Value),
    /// Close the connection without writing a response.
    Close,
    /// Write a response, then close the connection.
    CloseAfter(Value),
}

pub type MpvHandler = Arc<dyn Fn(Value) -> Reply + Send + Sync>;

/// Collect request `command` arrays until `count` are available or a short
/// deadline passes, making assertions robust against the server thread's
/// asynchronous delivery.
pub fn wait_commands(received: &mpsc::Receiver<Value>, count: usize) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut cmds = Vec::new();
    while cmds.len() < count && Instant::now() < deadline {
        cmds.extend(
            received
                .try_iter()
                .filter_map(|r| r.get("command").cloned()),
        );
        if cmds.len() < count {
            thread::sleep(Duration::from_millis(5));
        }
    }
    cmds
}

pub fn mpv_ok(data: Value) -> Value {
    json!({ "error": "success", "data": data })
}

pub fn mpv_err(msg: &str) -> Value {
    json!({ "error": msg })
}

/// Build a handler that always responds with a value produced by `f`.
pub fn handler_fn<F>(f: F) -> MpvHandler
where
    F: Fn(Value) -> Value + Send + Sync + 'static,
{
    Arc::new(move |req| Reply::Respond(f(req)))
}

/// Serializes tests that mutate process-wide environment variables.
pub static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Run `f` with the given environment variables temporarily set (or removed
/// when the value is `None`), restoring the previous values afterwards. Even
/// if `f` panics, the previous values are restored and the lock stays usable.
pub fn with_env<R>(envs: &[(&str, Option<&str>)], f: impl FnOnce() -> R) -> R {
    let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let saved: Vec<(String, Option<String>)> = envs
        .iter()
        .map(|(k, _)| (k.to_string(), std::env::var(k).ok()))
        .collect();
    for (k, v) in envs {
        match v {
            Some(val) => unsafe { std::env::set_var(k, val) },
            None => unsafe { std::env::remove_var(k) },
        }
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    for (k, v) in saved {
        match v {
            Some(val) => unsafe { std::env::set_var(&k, &val) },
            None => unsafe { std::env::remove_var(&k) },
        }
    }
    drop(guard);
    match result {
        Ok(r) => r,
        Err(p) => std::panic::resume_unwind(p),
    }
}

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Unique existing temp directory, cleaned up on drop.
pub struct Temp {
    dir: PathBuf,
}

impl Temp {
    pub fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "mpvd-{tag}-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    pub fn join(&self, p: impl AsRef<Path>) -> PathBuf {
        self.dir.join(p)
    }

    pub fn sock(&self) -> PathBuf {
        self.dir.join("mpvd.sock")
    }

    pub fn pid(&self) -> PathBuf {
        self.dir.join("mpvd.pid")
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// A fake mpv IPC server speaking the JSON-lines protocol over a Unix socket.
pub struct FakeServer {
    shut_tx: mpsc::Sender<()>,
    handle: Option<thread::JoinHandle<()>>,
    pub sock_path: PathBuf,
    pub received: mpsc::Receiver<Value>,
    _dir: Temp,
}

impl FakeServer {
    pub fn start(handler: MpvHandler) -> Self {
        Self::start_at(Temp::new("ipc"), handler)
    }

    pub fn start_at(dir: Temp, handler: MpvHandler) -> Self {
        let sock_path = dir.sock();
        let listener = UnixListener::bind(&sock_path).unwrap();
        listener.set_nonblocking(true).ok();
        let (shut_tx, shut_rx) = mpsc::channel::<()>();
        let (tx, rx) = mpsc::channel::<Value>();
        let handle = thread::spawn(move || {
            loop {
                if shut_rx.try_recv().is_ok() {
                    break;
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        let tx = tx.clone();
                        let h = handler.clone();
                        thread::spawn(move || handle_stream(stream, tx, h));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            shut_tx,
            handle: Some(handle),
            sock_path,
            received: rx,
            _dir: dir,
        }
    }

    /// Set MPVD_SOCK to this server and MPVD_PID to a file containing the
    /// current process's PID, so `daemon::start()` returns early and mpv IPC
    /// commands hit the fake server.
    pub fn with_env<R>(&self, f: impl FnOnce() -> R) -> R {
        let pid_file = self._dir.join("test.pid");
        std::fs::write(&pid_file, format!("{}\n", std::process::id())).unwrap();
        with_env(
            &[
                ("MPVD_SOCK", Some(self.sock_path.to_str().unwrap())),
                ("MPVD_PID", Some(pid_file.to_str().unwrap())),
            ],
            f,
        )
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        let _ = self.shut_tx.send(());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn handle_stream(stream: UnixStream, tx: mpsc::Sender<Value>, h: MpvHandler) {
    let Ok(reader) = stream.try_clone() else {
        return;
    };
    let mut writer = stream;
    for line in BufReader::new(reader).lines() {
        let Ok(line) = line else { break };
        let Ok(req) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let _ = tx.send(req.clone());
        let id = req.get("request_id").cloned().unwrap_or(Value::Null);
        match h(req) {
            Reply::Respond(mut r) => {
                r["request_id"] = id;
                let _ = writeln!(writer, "{r}");
            }
            Reply::Close => break,
            Reply::CloseAfter(mut r) => {
                r["request_id"] = id;
                let _ = writeln!(writer, "{r}");
                break;
            }
        }
    }
}
