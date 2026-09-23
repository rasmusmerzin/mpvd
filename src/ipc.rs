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

    pub fn observe(&mut self, property: &str) -> Result<u32, String> {
        self._id_seq += 1;
        write_msg(
            &mut self.stream,
            &json!({ "command": ["observe_property", self._id_seq, property] }),
        )
        .map(|_| self._id_seq)
    }

    pub fn poll(&self) -> Vec<(u32, String, Value)> {
        self.rx.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{FakeServer, Reply, handler_fn, mpv_err, mpv_ok};
    use serde_json::json;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn with_sock(sock: &str, f: impl FnOnce()) {
        crate::test_util::with_env(&[("MPVD_SOCK", Some(sock))], f);
    }

    #[test]
    fn parse_arg_parses_json_and_plain() {
        assert_eq!(parse_arg("5"), Value::from(5));
        assert_eq!(parse_arg("true"), Value::Bool(true));
        assert_eq!(parse_arg("\"quoted\""), Value::String("quoted".into()));
        assert_eq!(parse_arg("{\"a\":1}"), json!({ "a": 1 }));
        assert_eq!(parse_arg("not json"), Value::String("not json".into()));
        assert_eq!(parse_arg(""), Value::String("".into()));
    }

    #[test]
    fn send_raw_returns_matching_response() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!(42))));
        server.with_env(|| {
            let resp = send_raw(&[json!("get_property"), json!("x")]).unwrap();
            assert_eq!(resp["error"], "success");
            assert!(resp.get("request_id").is_some());
        });
        let req = server.received.try_iter().next().unwrap();
        assert_eq!(req["command"][0], "get_property");
        assert!(req.get("request_id").is_some());
    }

    #[test]
    fn send_extracts_data() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(json!(7))));
        server.with_env(|| {
            assert_eq!(
                send(&[json!("get_property"), json!("x")]).unwrap(),
                json!(7)
            );
        });
    }

    #[test]
    fn send_propagates_error() {
        let server = FakeServer::start(handler_fn(|_| mpv_err("property not found")));
        server.with_env(|| {
            let err = send(&[json!("get_property"), json!("nope")]).unwrap_err();
            assert!(err.contains("property not found"));
        });
    }

    #[test]
    fn send_of_null_data_returns_null() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(Value::Null)));
        server.with_env(|| {
            assert_eq!(
                send(&[json!("get_property"), json!("x")]).unwrap(),
                Value::Null
            );
        });
    }

    #[test]
    fn send_raw_errors_when_no_response() {
        let server = FakeServer::start(Arc::new(|_| Reply::Close));
        server.with_env(|| {
            let err = send_raw(&[json!("nop")]).unwrap_err();
            assert!(err.contains("no response from mpv"));
        });
    }

    #[test]
    fn send_fails_to_connect() {
        let sock = std::env::temp_dir().join(format!("mpvd-no-sock-{}", std::process::id()));
        with_sock(sock.to_str().unwrap(), || {
            let err = send(&[json!("get_property")]).unwrap_err();
            assert!(err.contains("failed to connect"));
        });
    }

    #[test]
    fn observe_prints_property_changes_and_stops_on_close() {
        let server = FakeServer::start(Arc::new(|_| {
            Reply::CloseAfter(json!({
                "event": "property-change",
                "id": 1,
                "name": "pause",
                "data": true,
            }))
        }));
        server.with_env(|| {
            assert!(observe("pause").is_ok());
        });
    }

    #[test]
    fn observer_connects_observes_and_polls() {
        let server = FakeServer::start(Arc::new(|_| {
            Reply::Respond(json!({
                "event": "property-change",
                "id": 1,
                "name": "playlist",
                "data": json!([{"filename": "/a.mp3"}]),
            }))
        }));
        server.with_env(|| {
            let mut observer = Observer::connect().unwrap();
            let id = observer.observe("playlist").unwrap();
            assert_eq!(id, 1);

            let mut got = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(2);
            while got.is_empty() && Instant::now() < deadline {
                got = observer.poll();
                std::thread::sleep(Duration::from_millis(5));
            }
            assert_eq!(got.len(), 1);
            assert_eq!(got[0].0, 1);
            assert_eq!(got[0].1, "playlist");
            assert_eq!(got[0].2, json!([{"filename": "/a.mp3"}]));
        });
    }

    #[test]
    fn observer_ignores_non_property_events() {
        let server = FakeServer::start(Arc::new(|_| {
            Reply::Respond(json!({
                "event": "start-file",
                "id": 0,
                "data": null,
            }))
        }));
        server.with_env(|| {
            let observer = Observer::connect().unwrap();
            std::thread::sleep(Duration::from_millis(20));
            assert!(observer.poll().is_empty());
        });
    }

    #[test]
    fn observer_connect_fails_without_socket() {
        let sock =
            std::env::temp_dir().join(format!("mpvd-observer-no-sock-{}", std::process::id()));
        with_sock(sock.to_str().unwrap(), || {
            assert!(Observer::connect().is_err());
        });
    }

    #[test]
    fn connect_failure_message() {
        let sock = std::env::temp_dir().join(format!("mpvd-connect-fail-{}", std::process::id()));
        with_sock(sock.to_str().unwrap(), || {
            let err = connect().unwrap_err();
            assert!(err.contains("failed to connect"));
        });
    }

    #[test]
    fn request_ids_are_monotonic() {
        let server = FakeServer::start(handler_fn(|_| mpv_ok(Value::Null)));
        server.with_env(|| {
            let before = REQUEST_ID.load(Ordering::Relaxed);
            let _ = send_raw(&[json!("a")]);
            let after = REQUEST_ID.load(Ordering::Relaxed);
            assert!(after > before);
        });
    }
}
