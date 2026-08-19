# mpvd

MPV music player daemon controller CLI.

## Build

```sh
cargo build --release
```

Binary is written to `./target/release/mpvd`.

## Commands

| Command               | Description                                |
| --------------------- | ------------------------------------------ |
| `mpvd init` (`start`) | Start an idle mpv daemon in the background |
| `mpvd kill`           | Kill the running mpv daemon                |
| `mpvd pid`            | Print the daemon's PID                     |
| `mpvd env`            | Print `MPVD_SOCK` and `MPVD_PID` paths     |

## Environment

- `MPVD_SOCK`: Path to the mpv IPC socket (default: `$XDG_RUNTIME_DIR/mpvd.sock` or `$HOME/mpvd.sock`)
- `MPVD_PID`: Path to the pid file (default: derived from `MPVD_SOCK`)

## Status

Work in progress. Currently implements daemon lifecycle commands only. Playback, playlist management, IPC, and interactive TUI are not yet ported from the [TypeScript version](https://github.com/merzin/mpvd).
