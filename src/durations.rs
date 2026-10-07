use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread;

use dashmap::DashMap;

use crate::control;

const WORKERS: usize = 4;

static CACHE: LazyLock<DashMap<String, f64>> = LazyLock::new(DashMap::new);
static PROBER: LazyLock<Sender<String>> = LazyLock::new(start_prober);

fn start_prober() -> Sender<String> {
    let (tx, rx) = mpsc::channel::<String>();
    let rx = Arc::new(Mutex::new(rx));
    for _ in 0..WORKERS {
        let rx = rx.clone();
        thread::spawn(move || {
            loop {
                let next = rx.lock().unwrap().recv();
                let Ok(filename) = next else { break };
                let d = control::probe_duration(&filename).unwrap_or(f64::NAN);
                CACHE.insert(filename, d);
            }
        });
    }
    tx
}

/// Cached duration of `path`, or `None` if unknown or probe failed.
pub fn get(path: &str) -> Option<f64> {
    let d = *CACHE.get(path)?;
    d.is_finite().then_some(d)
}

/// Enqueue a probe unless one is already cached or pending.
/// Inserts a NaN sentinel immediately so repeat calls are cheap no-ops.
pub fn request(path: &str) {
    if CACHE.contains_key(path) {
        return;
    }
    CACHE.insert(path.to_string(), f64::NAN);
    PROBER.send(path.to_string()).ok();
}

#[cfg(test)]
pub fn seed(path: &str, d: f64) {
    CACHE.insert(path.to_string(), d);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_unknown_path_is_none() {
        assert_eq!(get("dur-test-unknown.mp3"), None);
    }

    #[test]
    fn seed_roundtrips_and_filters_non_finite() {
        seed("dur-test-roundtrip.mp3", 123.5);
        assert_eq!(get("dur-test-roundtrip.mp3"), Some(123.5));
        seed("dur-test-nonfinite.mp3", f64::NAN);
        assert_eq!(get("dur-test-nonfinite.mp3"), None);
    }

    #[test]
    fn request_is_idempotent() {
        request("dur-test-idempotent.mp3");
        request("dur-test-idempotent.mp3");
        request("dur-test-idempotent.mp3");
        assert!(CACHE.contains_key("dur-test-idempotent.mp3"));
    }
}
