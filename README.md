# mpvd

MPV music player daemon controller CLI.

## Build

```sh
cargo build --release
```

Output binary will be placed at `target/release/mpvd`.

## Commands

### Daemon lifecycle

| Command               | Description                                |
| --------------------- | ------------------------------------------ |
| `mpvd init` (`start`) | Start an idle mpv daemon in the background |
| `mpvd kill`           | Kill the running mpv daemon                |
| `mpvd pid`            | Print the daemon's PID                     |
| `mpvd env`            | Print `MPVD_SOCK` and `MPVD_PID` paths     |

### Playlist management

| Command                | Description                                                              |
| ---------------------- | ------------------------------------------------------------------------ |
| `mpvd list` (`ls`)     | Show the playlist (`--plain` for raw names, `--full` for absolute paths) |
| `mpvd push <files...>` | Append one or more files to the playlist                                 |

### Playback

| Command                  | Description                                           |
| ------------------------ | ----------------------------------------------------- |
| `mpvd play [index]`      | Start/resume playback, optionally at a playlist index |
| `mpvd stop`              | Pause playback                                        |
| `mpvd next`              | Skip to the next track                                |
| `mpvd prev` (`previous`) | Go to the previous track                              |

### Raw IPC

| Command               | Description                                  |
| --------------------- | -------------------------------------------- |
| `mpvd send <cmd...>`  | Send arbitrary command to the mpv IPC socket |
| `mpvd observe <prop>` | Observe MPV property value                   |

## Environment

- `MPVD_SOCK`: Path to the mpv IPC socket (default: `$XDG_RUNTIME_DIR/mpvd.sock` or `$HOME/mpvd.sock`)
- `MPVD_PID`: Path to the pid file (default: derived from `MPVD_SOCK`)

## Status

Work in progress. Interactive TUI and remaining playlist operations are not yet ported from the [TypeScript version](https://github.com/merzin/mpvd).
