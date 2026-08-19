use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config;

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub fn send(command: &[Value]) -> Result<Value, String> {
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
            if let Some(err) = msg.get("error")
                && err != "success"
            {
                return Err(err.to_string());
            }
            return Ok(msg.get("data").cloned().unwrap_or(Value::Null));
        }
    }
    Err("no response from mpv".into())
}
