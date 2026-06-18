# mpvctl

MPV daemon controller CLI and TUI.

```sh
npm i -g @merzin/mpvctl
```

## mpvctl

```sh
mpvctl help
```

## mpvd

Legacy Bash script.

```sh
mpvd help
```

```
usage: mpvd [command=init] [args...]

commands:
  help                  Print this help message
  version               Print mpvd and mpv versions
  init                  Spawn mpv process or list running one
  stat                  List running mpv process
  kill                  Kill running mpv process
  push <file...>        Push files to playlist
  play [index]          Start playback
  stop                  Stop playback
  next                  Go to next file
  prev                  Go to previous file
  move <from> <to>      Move file in playlist
  remove <index>        Remove file at index from playlist
  pick [count=20]       Pick songs from random selection of count
  list                  Print playlist state
  time                  Print time position of current track
  position              Print playlist position of current track
  state                 Print playing/paused state

internal commands:
  send <cmd> [args...]  Send IPC command
  mesg <cmd> [args...]  Print IPC JSON message
  sock                  Print IPC socket path
```
