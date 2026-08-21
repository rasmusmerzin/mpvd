use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

use crate::config;

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

fn connect() -> Result<UnixStream, String> {
    UnixStream::connect(config::mpvd_sock()).map_err(|e| format!("failed to connect: {e}"))
}

fn write_msg(stream: &mut UnixStream, payload: &Value) -> Result<(), String> {
    stream
        .write_all(format!("{payload}\n").as_bytes())
        .map_err(|e| format!("failed to write: {e}"))
}

fn incoming(stream: &UnixStream) -> impl Iterator<Item = Value> + '_ {
    BufReader::new(stream)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
}

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
    let mut stream = connect()?;
    write_msg(
        &mut stream,
        &json!({ "command": command, "request_id": request_id }),
    )?;
    stream
        .shutdown(Shutdown::Write)
        .map_err(|e| format!("failed to shutdown: {e}"))?;
    incoming(&stream)
        .find(|msg| msg.get("request_id").and_then(|v| v.as_u64()) == Some(request_id))
        .ok_or_else(|| "no response from mpv".into())
}

pub fn parse_arg(arg: &str) -> Value {
    if let Ok(v) = serde_json::from_str::<Value>(arg) {
        return v;
    }
    Value::String(arg.to_string())
}

pub fn observe(property: &str) -> Result<(), String> {
    const OBSERVE_ID: u32 = 1;
    let mut stream = connect()?;
    write_msg(
        &mut stream,
        &json!({ "command": ["observe_property", OBSERVE_ID, property] }),
    )?;
    for msg in incoming(&stream) {
        if msg.get("event").and_then(|v| v.as_str()) == Some("property-change")
            && msg.get("id").and_then(|v| v.as_u64()) == Some(OBSERVE_ID as u64)
            && let Some(data) = msg.get("data")
        {
            println!("{data}");
        }
    }
    Ok(())
}

pub struct Observer {
    stream: UnixStream,
    rx: mpsc::Receiver<(u32, String, Value)>,
    _thread: std::thread::JoinHandle<()>,
    _id_seq: u32,
}

impl Observer {
    pub fn connect() -> Result<Self, String> {
        let stream = connect()?;
        let reader_stream = stream
            .try_clone()
            .map_err(|e| format!("failed to clone: {e}"))?;
        let (tx, rx) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            for msg in incoming(&reader_stream) {
                if msg.get("event").and_then(|v| v.as_str()) != Some("property-change") {
                    continue;
                }
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
        });
        Ok(Self {
            stream,
            rx,
            _thread: thread,
            _id_seq: 0,
        })
    }

    pub fn observe(&mut self, property: &str) -> Result<(), String> {
        self._id_seq += 1;
        write_msg(
            &mut self.stream,
            &json!({ "command": ["observe_property", self._id_seq, property] }),
        )
    }

    pub fn poll(&self) -> Vec<(u32, String, Value)> {
        self.rx.try_iter().collect()
    }
}
