# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.3] - 2026-02-11

### Fixed

- Updated README and CHANGELOG to reflect v0.1.2 API changes (missed in v0.1.2 release)
- README now documents `ActionFailed` event, `ActionKind` enum, and updated `Toggle` action signature

## [0.1.2] - 2026-02-11

### Added

- `ActionKind` enum for categorizing action failures (`Toggle`, `ConfigApply`, `SwitchMode`)
- `ActionFailed` event variant in `WlMonitorEvent` for structured error reporting
- Smart mode resolution in `configure_toggle`: remembers last active mode via `last_mode` field
- Optional custom mode parameter `(width, height, refresh_rate)` for `Toggle` action
- `last_mode` field on `WlMonitor` to store mode ID before disabling

### Changed

- Rewritten `configure_toggle` with smart mode resolution priority: custom mode > last mode > preferred mode > first available
- Improved error handling: failures emit `ActionFailed` events instead of silently failing

### Fixed

- Mode-to-monitor mapping cleanup on monitor removal

## [0.1.1] - 2026-02-10

### Fixed

- Updated repository URL in Cargo.toml
- Fixed authors field in Cargo.toml

## [0.1.0] - 2026-02-09

### Added

- Initial release
- Wayland output manager using wlr-output-management protocol
- Support for detecting connected monitors
- Support for toggling monitors on/off
- Support for switching monitor modes (resolution/refresh rate)
- Example application `monitor_info`
