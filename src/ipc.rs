use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config;

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub fn send(command: &[Value]) -> Result<Value, String> {
    let resp = send_raw(command)?;
    if let Some(err) = resp.get("error")
        && err != "success"
    {
        return Err(err.to_string());
    }
    Ok(resp.get("data").cloned().unwrap_or(Value::Null))
}

pub fn send_raw(command: &[Value]) -> Result<Value, String> {
    let request_id = REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let payload = json!({ "command": command, "request_id": request_id });
    let mut stream =
        UnixStream::connect(config::mpvd_sock()).map_err(|e| format!("failed to connect: {e}"))?;
    stream
        .write_all(format!("{payload}\n").as_bytes())
        .map_err(|e| format!("failed to write: {e}"))?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|e| format!("failed to shutdown: {e}"))?;
    let reader = BufReader::new(&stream);
    for line in reader.lines() {
        let line = line.map_err(|e| format!("failed to read: {e}"))?;
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if msg.get("request_id").and_then(|v| v.as_u64()) == Some(request_id) {
            return Ok(msg);
        }
    }
    Err("no response from mpv".into())
}

pub fn parse_arg(arg: &str) -> Value {
    if let Ok(v) = serde_json::from_str::<Value>(arg) {
        return v;
    }
    Value::String(arg.to_string())
}

pub fn observe(property: &str) -> Result<(), String> {
    let mut stream =
        UnixStream::connect(config::mpvd_sock()).map_err(|e| format!("failed to connect: {e}"))?;
    let observe_id = 1u32;
    let cmd = json!({ "command": ["observe_property", observe_id, property] });
    stream
        .write_all(format!("{cmd}\n").as_bytes())
        .map_err(|e| format!("failed to write: {e}"))?;
    let reader = BufReader::new(&stream);
    for line in reader.lines() {
        let line = line.map_err(|e| format!("failed to read: {e}"))?;
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if msg.get("event").and_then(|v| v.as_str()) == Some("property-change")
            && msg.get("id").and_then(|v| v.as_u64()) == Some(observe_id as u64)
            && let Some(data) = msg.get("data")
        {
            println!("{data}");
        }
    }
    Ok(())
}

pub struct Observer {
    stream: UnixStream,
    rx: std::sync::mpsc::Receiver<(u32, String, Value)>,
    _thread: std::thread::JoinHandle<()>,
}

impl Observer {
    pub fn connect() -> Result<Self, String> {
        let stream = UnixStream::connect(config::mpvd_sock())
            .map_err(|e| format!("failed to connect: {e}"))?;
        let reader_stream = stream
            .try_clone()
            .map_err(|e| format!("failed to clone: {e}"))?;
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let reader = BufReader::new(reader_stream);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break,
                };
                let msg: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if msg.get("event").and_then(|v| v.as_str()) == Some("property-change") {
                    let id = msg.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let name = msg
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let data = msg.get("data").cloned().unwrap_or(Value::Null);
                    if tx.send((id, name, data)).is_err() {
                        break;
                    }
                }
            }
        });
        Ok(Self {
            stream,
            rx,
            _thread: thread,
        })
    }

    pub fn observe(&mut self, id: u32, property: &str) -> Result<(), String> {
        let cmd = json!({ "command": ["observe_property", id, property] });
        self.stream
            .write_all(format!("{cmd}\n").as_bytes())
            .map_err(|e| format!("failed to write: {e}"))
    }

    pub fn unobserve(&mut self, id: u32) -> Result<(), String> {
        let cmd = json!({ "command": ["unobserve_property", id] });
        self.stream
            .write_all(format!("{cmd}\n").as_bytes())
            .map_err(|e| format!("failed to write: {e}"))
    }

    pub fn poll(&self) -> Vec<(u32, String, Value)> {
        self.rx.try_iter().collect()
    }
}
