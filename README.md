# ringmaster.rs

`ringmaster.rs` is a local-first Rust terminal application for exploring Oura Cloud data with a Ratatui interface, SQLite-backed local storage, deterministic demo mode, and poll-first integration seams for Oura Cloud API v2.

## Status

This repository now includes a real phase-0 / phase-1 foundation:

- `clap` CLI with `tui`, `doctor`, `auth login`, `sync once`, and `demo`
- `ratatui + crossterm + tokio` application shell
- deterministic demo data that exercises Dashboard, Timeline, Trends, and Ops
- SQLite bootstrap schema with migrations and typed store boundaries
- structured logging via `tracing`
- poll-first Oura auth/client/sync scaffolding with explicit partial-capability handling

The project is intentionally not feature-complete yet. The current goal is a serious shell with clean boundaries, not a one-shot full product dump.

## Commands

```bash
cargo run -- tui
cargo run -- doctor
cargo run -- auth login
cargo run -- sync once
cargo run -- demo
```

Rust toolchain baseline: `rust-version = 1.88`.

Behavior notes:

- `ringmaster tui` launches the live TUI when attached to a terminal. Without a TTY it renders a deterministic snapshot of the current local app state instead.
- `ringmaster demo` launches the same UI shell in deterministic demo mode. Without a TTY it renders a text snapshot, which is useful for CI and screenshot-oriented workflows.
- `ringmaster doctor` resolves paths, initializes the SQLite store, applies migrations, and prints health details.
- `ringmaster auth login` prepares an OAuth authorization URL and describes the loopback callback scaffold.
- `ringmaster sync once` records poll-first sync readiness and capability status in SQLite. Real endpoint imports land in the next milestone.

## Local-first layout

`ringmaster.rs` uses XDG-friendly paths by default:

- config directory: `$XDG_CONFIG_HOME/ringmaster` or `~/.config/ringmaster`
- config file: `config.toml`
- state directory: `$XDG_STATE_HOME/ringmaster` or `~/.local/state/ringmaster`
- database: `ringmaster.db`
- cache directory: `$XDG_CACHE_HOME/ringmaster` or `~/.cache/ringmaster`

The app creates runtime directories as needed, but it does not create a config file unless you do.

## Example config

Create `~/.config/ringmaster/config.toml`:

```toml
[logging]
filter = "ringmaster=debug"

[oura]
client_id = "your-oura-client-id"
client_secret = "your-oura-client-secret"
callback_bind = "127.0.0.1:8788"
callback_path = "/callback"
requested_scopes = ["personal", "daily", "heartrate", "workout", "session", "tag"]
```

Optional development-only override:

- `granted_scopes = ["personal", "daily"]`

That field exists so the bootstrap shell can surface partial capability states before token persistence is finished. It should be treated as a temporary bootstrap seam, not the long-term token source of truth.

## Architecture summary

The codebase stays in a single crate for now, with narrow module boundaries:

- `src/cli.rs`: CLI parsing and help text
- `src/config.rs`: config loading, XDG paths, runtime defaults
- `src/app.rs`: app state, screen models, demo/live data shaping
- `src/tui.rs`: Ratatui event loop and snapshot rendering
- `src/components/*`: pure rendering for Dashboard, Timeline, Trends, and Ops
- `src/store/*`: SQLite plan, migrations, typed store queries
- `src/oura/*`: OAuth planning, typed client interface, sync orchestration scaffolding

UI components do not perform network calls, token refresh, or database writes. The TUI reads presentation models only. Sync and auth are wired behind seams that can mature without forcing a TUI rewrite.

More detail lives in [docs/ARCHITECTURE.md](/home/ubuntu/ringmaster.rs/docs/ARCHITECTURE.md).

The storage backend choice is documented separately in [docs/decisions/20260408-storage-backend-rusqlite.md](/home/ubuntu/ringmaster.rs/docs/decisions/20260408-storage-backend-rusqlite.md).

## Verification

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
```

`cargo run -- demo` is also worth a quick smoke check, especially when changing layouts or screenshot flows.

## Next milestone

The next slice is to complete the OAuth callback capture and token persistence path, then turn `sync once` into a real poll-first import for daily summaries and heartrate data.
