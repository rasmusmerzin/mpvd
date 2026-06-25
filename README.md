# mpvd

MPV music player daemon controller CLI and TUI.

## Install

```sh
npm i -g @merzin/mpvd
```

> **Note:** `mpv` must be installed on your system.

## Usage

### Daemon lifecycle

| Command               | Description                                |
| --------------------- | ------------------------------------------ |
| `mpvd init` (`start`) | Start an idle mpv daemon in the background |
| `mpvd kill`           | Kill the running mpv daemon                |
| `mpvd pid`            | Print the daemon's PID                     |
| `mpvd env`            | Print `MPVD_SOCK` and `MPVD_PID` paths     |

The daemon runs as a headless mpv instance (`--idle --no-video`) and listens on
a Unix socket at `$MPVD_SOCK` (defaults to `$XDG_RUNTIME_DIR/mpvd.sock` or
`$HOME/mpvd.sock`). The PID is stored at `$MPVD_PID`.

### Playback

| Command             | Description                                           |
| ------------------- | ----------------------------------------------------- |
| `mpvd play [index]` | Start/resume playback, optionally at a playlist index |
| `mpvd stop`         | Pause playback                                        |
| `mpvd next`         | Skip to the next track                                |
| `mpvd prev`         | Go to the previous track                              |

### Playlist management

| Command                        | Description                                                                                                   |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------- |
| `mpvd list` (`ls`)             | Show the playlist (`--plain` for raw names, `--full` for absolute paths, `--interactive` for interactive TUI) |
| `mpvd push <files...>`         | Append one or more files to the playlist                                                                      |
| `mpvd insert <files...>`       | Insert one or more files to the playlist after current track                                                  |
| `mpvd pick [dirpath=~/Music]`  | Interactive TUI to browse and pick files                                                                      |
| `mpvd move <from> <to>` (`mv`) | Move a track from one playlist index to another                                                               |
| `mpvd remove <index>` (`rm`)   | Remove a track at the given playlist index                                                                    |
| `mpvd position` (`pos`)        | Print the playlist index of the current track (1-based)                                                       |

### Info

| Command        | Description                                                                                |
| -------------- | ------------------------------------------------------------------------------------------ |
| `mpvd time`    | Print current time position (`--seconds` for raw seconds, `--duration` for total duration) |
| `mpvd state`   | Print `paused` or `playing`                                                                |
| `mpvd current` | Print current track                                                                        |

### Raw IPC

| Command                   | Description                                  |
| ------------------------- | -------------------------------------------- |
| `mpvd send <cmd...>`      | Send arbitrary command to the mpv IPC socket |
| `mpvd observe <property>` | Observe MPV property                         |

The `send` command accepts JSON-native arguments (strings, numbers, booleans)
and prints the raw response from mpv. This is useful for sending any
[mpv IPC command](https://mpv.io/manual/stable/#json-ipc).

### Interactive picker (`mpvd pick`)

Opens a full-screen, keyboard-driven file browser rooted at `~/Music`
(configurable via the `[dirpath]` argument). Browse recursively, select multiple
audio files, search by regex, shuffle, and push them to the playlist.

**Keybindings:**

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

**Search mode** keybindings:

| Key                  | Action                          |
| -------------------- | ------------------------------- |
| `Enter`              | Apply search filter             |
| `Esc`                | Cancel search                   |
| `←`/`Ctrl-b`         | Move cursor left                |
| `→`/`Ctrl-f`         | Move cursor right               |
| `Backspace`/`Ctrl-h` | Delete character before cursor  |
| `Delete`/`Ctrl-d`    | Delete character at cursor      |
| `Ctrl-a`             | Go to beginning of line         |
| `Ctrl-e`             | Go to end of line               |
| `Ctrl-u`             | Delete everything before cursor |
| `Ctrl-k`             | Delete everything after cursor  |
| `Ctrl-w`             | Delete word before cursor       |

### Interactive playlist (`mpvd list --interactive`)

Opens a full-screen interactive playlist browser. Playlist state is polled in
real time.

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
| `←`/`Ctrl-b`       | Seek 5 seconds backward                             |
| `→`/`Ctrl-f`       | Seek 5 seconds forward                              |

## Environment

- `MPVD_SOCK` — path to the mpv IPC socket (default:
  `$XDG_RUNTIME_DIR/mpvd.sock` or `$HOME/mpvd.sock`)
- `MPVD_PID` — path to the pid file (derived from `MPVD_SOCK`)

---

## mpvctl

Legacy bash wrapper with the same IPC interface. Requires `socat`, `jq`, `fzf`
and `bat`.

```sh
mpvctl help
```

Run `mpvctl help` to list all commands, or use the same subcommands as `mpvd`
(e.g. `mpvctl init`, `mpvctl push`, `mpvctl play`, etc.).
