# Changelog

All notable changes to PARAMO are documented here.  
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [Unreleased]

### Added
- Streak system (`racha`): tracks consecutive clean days without unblocking during schedule
- Blocked attempt counter (`intentos`): counts how many times the user tried to unblock during active schedule hours
- Prebuilt Homebrew bottles for arm64 (Apple Silicon) and x86_64 (Intel) — `brew install` no longer requires compiling Rust from source
- Automated release workflow: pushing a tag builds bottles, creates the GitHub Release, and updates the formula automatically

### Changed
- Split `tui.rs` into a proper module structure (`tui/mod.rs`, `state.rs`, `render.rs`, `input.rs`, `animations.rs`, `helpers.rs`)
- Moved i18n string literals to external TOML files (`locales/es.toml`, `locales/en.toml`) — strings can now be edited without touching Rust code
- IPC listener now uses a bounded worker pool (4 workers) instead of spawning an unbounded number of threads per connection
- Improved IPC error logging: failed `block`, `unblock`, and `sync` operations now log the specific error

---

## [0.1.0] — 2026-03-01

First public release. Distributed via `brew tap attinto/paramo`.

### Added
- CLI with subcommands: `status`, `block`, `unblock`, `site`, `schedule`, `lang`, `config`, `doctor`, `install`, `uninstall`
- Interactive TUI with tabs: Home, Sites, Schedule, Settings, Diagnostics, Wall, Exit
- ASCII art header and animated distraction phrases
- Blocking via `/etc/hosts` redirect to `127.0.0.1` and `::1`
- macOS launchd daemon (`com.paramo.blocker`) for automatic schedule enforcement
- IPC socket at `/var/run/paramo.sock` for user→daemon communication
- Schedule configuration: start/end hour, optional weekend blocking
- Spanish and English UI (`paramo lang set es|en`)
- Anti-stop-blocker friction: 30-second countdown before unblocking during schedule
- Mandatory reason prompt before unblocking
- 60-second breathing animation while contemplating the unblock
- Wall of Shame: persistent journal of unblock reasons (`~/.config/paramo/journal.log`)
- `paramo doctor` diagnostics with actionable hints
- DNS-over-HTTPS warning (browsers with DoH bypass `/etc/hosts`)
- Homebrew formula with smoke tests
- GitHub Actions CI: `cargo test`, `clippy -D warnings`, formula validation
