# mpvd

MPV music player daemon controller CLI.

## Build

```sh
cargo build --release
```

Output binary will be placed at `target/release/mpvd`.

## Usage

Running `mpvd` with no arguments opens the interactive playlist.

## Commands

### Daemon lifecycle

| Command               | Description                                |
| --------------------- | ------------------------------------------ |
| `mpvd init` (`start`) | Start an idle mpv daemon in the background |
| `mpvd kill`           | Kill the running mpv daemon                |
| `mpvd pid`            | Print the daemon's PID                     |
| `mpvd env`            | Print `MPVD_SOCK` and `MPVD_PID` paths     |

### Playlist management

| Command                        | Description                                                                                                   |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------- |
| `mpvd list` (`ls`)             | Show the playlist (`--plain` for raw names, `--full` for absolute paths, `--interactive` for interactive TUI) |
| `mpvd push <files...>`         | Append one or more files to the playlist                                                                      |
| `mpvd insert <files...>`       | Insert one or more files to the playlist after current track                                                  |
| `mpvd move <from> <to>` (`mv`) | Move a track from one playlist index to another                                                               |
| `mpvd remove <index>` (`rm`)   | Remove a track at the given playlist index                                                                    |
| `mpvd position` (`pos`)        | Print the playlist index of the current track (1-based)                                                       |

### Playback

| Command                  | Description                                           |
| ------------------------ | ----------------------------------------------------- |
| `mpvd play [index]`      | Start/resume playback, optionally at a playlist index |
| `mpvd stop`              | Pause playback                                        |
| `mpvd next`              | Skip to the next track                                |
| `mpvd prev` (`previous`) | Go to the previous track                              |

### Info

| Command        | Description                                                                                |
| -------------- | ------------------------------------------------------------------------------------------ |
| `mpvd time`    | Print current time position (`--seconds` for raw seconds, `--duration` for total duration) |
| `mpvd state`   | Print `paused` or `playing`                                                                |
| `mpvd current` | Print current track                                                                        |

### Raw IPC

| Command               | Description                                  |
| --------------------- | -------------------------------------------- |
| `mpvd send <cmd...>`  | Send arbitrary command to the mpv IPC socket |
| `mpvd observe <prop>` | Observe MPV property value                   |

### Interactive

| Command               | Description                                  |
| --------------------- | -------------------------------------------- |
| `mpvd pick [dirpath]` | Interactive file picker (default: `~/Music`) |

## Interactive playlist (`mpvd` / `mpvd list --interactive`)

Opens a full-screen interactive playlist browser. Playlist state is polled in real time.

### Keybindings

| Key                | Action                                              |
| ------------------ | --------------------------------------------------- |
| `↑`/`k`/`Ctrl-p`   | Move cursor up                                      |
| `↓`/`j`/`Ctrl-n`   | Move cursor down                                    |
| `Ctrl-u`           | Scroll up half a page                               |
| `Ctrl-d`           | Scroll down half a page                             |
| `Ctrl-y`           | Scroll up one line                                  |
| `Ctrl-e`           | Scroll down one line                                |
| `g`                | Go to top                                           |
| `G`                | Go to bottom                                        |
| `H`                | Go to top of visible page                           |
| `L`                | Go to bottom of visible page                        |
| `Enter`            | Play highlighted track (or toggle pause if current) |
| `Space`            | Toggle play/pause                                   |
| `f`                | Toggle display between basename and absolute path   |
| `J`/`Shift+↓`      | Move track down in playlist                         |
| `K`/`Shift+↑`      | Move track up in playlist                           |
| `D`/`Delete`       | Remove track from playlist                          |
| `p`                | Open picker to add files                            |
| `q`/`Esc`/`Ctrl-c` | Exit interactive list                               |
| `Ctrl-←`/`Ctrl-b`  | Seek 5 seconds backward                             |
| `Ctrl-→`/`Ctrl-f`  | Seek 5 seconds forward                              |

## Interactive picker (`mpvd pick`)

Opens a full-screen file browser rooted at `~/Music` (configurable via the `[dirpath]` argument). Browse recursively, select multiple audio files, search by regex, shuffle, and push them to the playlist.

### Keybindings

| Key                | Action                                            |
| ------------------ | ------------------------------------------------- |
| `↑`/`k`/`Ctrl-p`   | Move cursor up                                    |
| `↓`/`j`/`Ctrl-n`   | Move cursor down                                  |
| `Ctrl-u`           | Scroll up half a page                             |
| `Ctrl-d`           | Scroll down half a page                           |
| `Ctrl-y`           | Scroll up one line                                |
| `Ctrl-e`           | Scroll down one line                              |
| `g`                | Go to top                                         |
| `G`                | Go to bottom                                      |
| `H`                | Go to top of visible page                         |
| `L`                | Go to bottom of visible page                      |
| `Space`/`Tab`      | Toggle selection of highlighted file              |
| `i`                | Mark highlighted file for insertion               |
| `Enter`            | Push selected files to playlist and exit          |
| `r`                | Toggle shuffle                                    |
| `f`                | Toggle display between basename and absolute path |
| `/`                | Enter search mode                                 |
| `q`/`Esc`/`Ctrl-c` | Exit without adding files                         |

## Environment

- `MPVD_SOCK`: Path to the mpv IPC socket (default: `$XDG_RUNTIME_DIR/mpvd.sock` or `$HOME/mpvd.sock`)
- `MPVD_PID`: Path to the pid file (default: derived from `MPVD_SOCK`)
