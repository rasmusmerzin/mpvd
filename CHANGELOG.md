# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0),
and this project adheres to
[Semantic Versioning](https://docs.npmjs.com/about-semantic-versioning).

## [Unreleased]

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

[unreleased]: https://github.com/rasmusmerzin/mpvd/compare/v0.4.0...main
[0.4.0]: https://github.com/rasmusmerzin/mpvd/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/rasmusmerzin/mpvd/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/rasmusmerzin/mpvd/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/rasmusmerzin/mpvd/tree/v0.1.0
