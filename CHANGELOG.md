# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0),
and this project adheres to
[Semantic Versioning](https://docs.npmjs.com/about-semantic-versioning).

## [Unreleased]

## [2.2.0] - 2026-08-23

### Changed

- `mpvd push` starts the daemon if not already started.
- `mpvd init` probes readiness of socket and waits until socket is ready.
- `mpvd pick` ignores hidden files.

### Fixed

- `mpvd pick` ignores cyclical links.
- `mpvd ls | head` will not panic.
- `mpvd pick` empty search results will not panic.
- Replace hardcoded observe ids with generated ones in interactive playlist.
- Update scroll and cursor when playlist is modified externally.

## [2.1.1] - 2026-08-23

### Changed

- `mpvd pick` sorts audio files by name.

## [2.1.0] - 2026-08-23

### Added

- Supervisor to `mpv` daemon process. `MPVD_PID` and `MPVD_SOCK` are removed
  when `mpv` is killed directly as well.

## [2.0.0] - 2026-08-21

### Changed

- Rewritten in Rust.

## [1.0.0] - 2026-06-26

### Added

- `pick` keybind for marking for insertion
- `insert` subcommand
- `observe` subcommand

### Changed

- Added error handling to `send` function
- Configured interactive list to use observer pattern instead of polling

## [0.5.0] - 2026-06-25

### Added

- `list` is now the default command; running `mpvd` with no arguments opens the
  interactive playlist
- `list` left and right arrow keys keybinds to seek
- `list` shift arrow keybinds for moving playlist entries
- Use given `MPVD_PID` if provided

## [0.4.0] - 2026-06-24

### Added

- `list` subcommand `-i`/`--interactive` flag

## [0.3.0] - 2026-06-24

### Added

- `pick` directory path argument
- `start` alias for `init` subcommand

## [0.2.0] - 2026-06-23

### Added

- `state` subcommand
- `current` subcommand

## [0.1.0] - 2026-06-22

### Added

- `init` subcommand
- `kill` subcommand
- `pid` subcommand
- `env` subcommand
- `list`/`ls` subcommand
- `position`/`pos` subcommand
- `time` subcommand
- `play` subcommand
- `stop` subcommand
- `push` subcommand
- `pick` subcommand
- `next` subcommand
- `prev`/`previous` subcommand
- `move`/`mv` subcommand
- `remove`/`rm` subcommand
- `send` subcommand

[unreleased]: https://github.com/rasmusmerzin/mpvd/compare/v2.2.0...main
[2.2.0]: https://github.com/rasmusmerzin/mpvd/compare/v2.1.1...v2.2.0
[2.1.1]: https://github.com/rasmusmerzin/mpvd/compare/v2.1.0...v2.1.1
[2.1.0]: https://github.com/rasmusmerzin/mpvd/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/rasmusmerzin/mpvd/compare/v1.0.0...v2.0.0
[1.0.0]: https://github.com/rasmusmerzin/mpvd/compare/v0.5.0...v1.0.0
[0.5.0]: https://github.com/rasmusmerzin/mpvd/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/rasmusmerzin/mpvd/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/rasmusmerzin/mpvd/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/rasmusmerzin/mpvd/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/rasmusmerzin/mpvd/tree/v0.1.0
