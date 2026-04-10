# Contributing to PARAMO

Thanks for your interest. PARAMO is a small personal tool, but contributions are welcome — especially bug fixes, translations, and features that fit the original scope (focused work, macOS, CLI/TUI).

## What we're looking for

- Bug fixes and edge case handling
- New languages in `locales/` (just add a new `.toml` file and wire it up in `i18n.rs`)
- Features that stay in scope: blocking, scheduling, friction flows

What we're probably NOT looking for: Linux/Windows ports, GUI apps, browser extensions, cloud sync, or anything that requires external services.

## Setup

You'll need Rust (stable) and a Mac to test system integration. The TUI and CLI work fine without root — only `paramo install`, `block`, and `unblock` need it.

```bash
git clone https://github.com/attinto/homebrew-paramo
cd homebrew-paramo
cargo build
cargo test
```

To test the full daemon flow you'll need macOS and `sudo`.

## Making changes

1. Fork the repo and create a branch from `master`
2. Keep changes focused — one thing per PR
3. Run `cargo clippy` and make sure it passes with no warnings
4. If you're adding a new feature, update `CHANGELOG.md` under `[Unreleased]`
5. If you're adding/changing strings, edit `locales/es.toml` and `locales/en.toml` — not `i18n.rs`

## Releases and Homebrew bottles

Releases are fully automated. Pushing a `vX.Y.Z` tag triggers a GitHub Actions workflow that:

- Compiles prebuilt bottles for arm64 (Apple Silicon) and x86_64 (Intel)
- Creates the GitHub Release with the bottle assets
- Updates `Formula/paramo.rb` with the new tag, revision, and SHA256 hashes

You don't need to touch the formula manually. The only prerequisite is that the repo has **Read and write permissions** enabled under `Settings → Actions → General → Workflow permissions`.

## Adding a new language

1. Copy `locales/es.toml` to `locales/xx.toml` (where `xx` is the language code)
2. Translate all the values — keys and parameter markers (`{0}`, `{1}`) must stay as-is
3. Add the variant to the `Language` enum in `src/i18n.rs`
4. Add it to `Language::parse()` and register it in `translations()` in the same file
5. Update `Language::code()` and `Language::native_name()`

## Code style

- Standard `rustfmt` formatting (`cargo fmt`)
- No `unwrap()` in production paths — use `?` or handle the error explicitly
- Comments only where the logic isn't obvious on its own
- Keep files under ~300 lines where possible

## Commit messages

Short imperative sentences work fine: `fix schedule wrap-around at midnight`, `add French translation`, `improve doctor output for launchd errors`.

No need for conventional commits or ticket numbers.

## Questions?

Open an issue. If something's broken, include the output of `paramo doctor`.
