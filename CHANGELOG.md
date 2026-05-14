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
- Friction flow when removing a site from the configuration (TUI and CLI): same 30 s + mandatory reason + 60 s process used by unblock. Site removals are also recorded in the wall of shame, but they do not break the streak.
- New `journal::append_site_removal` for site-removal entries, kept separate from the streak-breaking `journal::append`.

### Changed
- Split `tui.rs` into a proper module structure (`tui/mod.rs`, `state.rs`, `render.rs`, `input.rs`, `animations.rs`, `helpers.rs`)
- Moved i18n string literals to external TOML files (`locales/es.toml`, `locales/en.toml`) — `doctor` and `install` strings now live there too
- IPC listener now uses a bounded worker pool (4 workers) instead of spawning an unbounded number of threads per connection
- Improved IPC error logging: failed `block`, `unblock`, and `sync` operations now log the specific error
- Generalised the unblock friction state machine into a generic `FrictionFlow` so the same machinery powers the new site-removal flow

### Fixed
- `config/default.toml` advertised `interval_seconds = 60` while the docs and the in-code default said 1200; aligned everything to 1200 s.
- `expand_domains` no longer prepends `www.` to subdomains like `m.tiktok.com`, where the `www.` form does not exist.
- The daemon no longer credits a "clean day" on startup; the streak only advances when the daemon actually observes a day rollover at runtime.
- Bumped the Intel runner in `release.yml` from the retired `macos-12` to `macos-13`.

### Removed
- Vendored `serde_json` shim under `vendor/serde_json` (340-line custom JSON parser/serialiser backed by `toml::Value`). Replaced with the upstream `serde_json` crate from crates.io. Existing JSON state files keep working: real `serde_json` reads what the shim wrote.

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
