# ringmaster.rs

`ringmaster.rs` is a local-first Rust terminal application for exploring Oura Cloud data with a Ratatui interface, SQLite-backed local storage, deterministic demo and fixture paths, and a poll-first Oura Cloud API v2 integration.

## Status

This repository now includes a context-aware, daily-drivable personal observability MVP:

- `clap` CLI with `tui`, `tui --demo`, `doctor`, `auth login`, `sync once`, `sync watch`, `derive rebuild`, and the compatibility alias `demo`
- a useful Ratatui Dashboard, Timeline, Trends, Explain, Patterns, and Ops screen backed by persisted SQLite data
- deterministic demo data that exercises the same six-screen shell without credentials or network access
- real loopback OAuth login with server-side code exchange, PKCE, and CSRF-safe state handling
- persisted auth/session metadata in SQLite with token secrets stored through the OS keyring seam
- real poll-first sync for personal info, daily summaries, heartrate, workouts, enhanced tags, and sessions into normalized tables plus raw payload cache
- persisted derived read models for canonical context events and deterministic pattern summaries, refreshed automatically after successful syncs
- family-aware background refresh while the TUI is open, plus the same scheduler exposed as `sync watch`
- explicit freshness and availability semantics for fresh, stale, missing scope, no data yet, never synced, auth failure, and source-delayed data
- a restrained, deterministic explainability layer with selected-day summaries, evidence bullets, thin-data notes, and pattern summaries
- structured logging via `tracing`

The project is intentionally not feature-complete yet. The goal is a trustworthy local foundation with one real observability vertical slice, not a one-shot full product dump.

## Commands

```bash
cargo run -- tui
cargo run -- tui --demo
cargo run -- doctor
cargo run -- auth login
cargo run -- sync once
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3
cargo run -- sync watch
cargo run -- sync watch --demo --max-iterations 1
cargo run -- derive rebuild
cargo run -- derive rebuild --demo
cargo run -- demo
```

Rust toolchain baseline: `rust-version = 1.88`.

Behavior notes:

- `ringmaster tui` launches the live TUI when attached to a terminal. Without a TTY it renders a snapshot of the current local app state instead.
- `ringmaster tui --demo` launches the same UI shell in deterministic demo mode. Without a TTY it renders a text snapshot, which is useful for CI and smoke-oriented workflows.
- `ringmaster demo` remains as a compatibility alias for `ringmaster tui --demo`.
- `ringmaster doctor` resolves paths, initializes SQLite, applies migrations, and prints auth, capability, per-family freshness, refresh policy, record-count, and path diagnostics.
- `ringmaster auth login` starts a loopback OAuth flow, validates state, exchanges the code server-side, and persists auth/session metadata locally.
- `ringmaster sync once` refreshes auth when needed, imports all supported sync families, caches raw payloads, upserts normalized SQLite rows, and refreshes the derived context/pattern tables when the underlying persisted data changes.
- `ringmaster sync once --dry-run --fixture-dir tests/fixtures/phase3` exercises the same normalization pipeline without live credentials or database writes. This is the bounded fixture-backed equivalent of a demo sync smoke path.
- `ringmaster sync watch` runs the same family-aware scheduler used by the live TUI, but without the UI.
- `ringmaster sync watch --demo --max-iterations 1` is the bounded scheduler smoke path for CI and local verification. It uses the checked-in fixtures by default and exits after one scheduler iteration.
- `ringmaster derive rebuild` deterministically rebuilds derived context events and pattern summaries from persisted SQLite data.
- `ringmaster derive rebuild --demo` seeds a temporary store from the phase-3 fixtures, rebuilds derived state, prints a compact summary, and exits without requiring live credentials.

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
callback_bind = "127.0.0.1:8788"
callback_path = "/callback"
requested_scopes = [
  "personal",
  "daily",
  "heartrate",
  "workout",
  "enhanced_tag",
  "session",
]

[refresh]
personal_interval_secs = 3600
daily_interval_secs = 300
heartrate_interval_secs = 60
workout_interval_secs = 600
enhanced_tag_interval_secs = 300
session_interval_secs = 300

personal_stale_after_secs = 259200
daily_stale_after_secs = 43200
heartrate_stale_after_secs = 900
workout_stale_after_secs = 86400
enhanced_tag_stale_after_secs = 43200
session_stale_after_secs = 43200

daily_history_days = 90
daily_overlap_days = 2
heartrate_history_days = 7
heartrate_overlap_minutes = 60
workout_history_days = 90
workout_overlap_days = 2
enhanced_tag_history_days = 90
enhanced_tag_overlap_days = 2
session_history_days = 90
session_overlap_days = 2
```

Set the client secret in the environment instead of the config file:

```bash
export RINGMASTER_OURA_CLIENT_SECRET="your-oura-client-secret"
```

Important notes:

- granted scopes are not configured in `config.toml`; they come from the persisted auth session
- token secrets are intentionally kept out of plaintext config
- live auth and sync are local-first only; there is no required webhook infrastructure for the first usable release

## Supported families

The current live sync and persistence surface includes:

- `personal`
- `daily`
- `heartrate`
- `workout`
- `enhanced_tag`
- `session`

Legacy `tags` remain read-compatible in the derived event layer when they already exist in the database, but the product is intentionally centered on `enhanced_tag` for this pass.

## Product behavior

What the screens now do:

- Dashboard shows the shared selected day, daily metric cards, freshness badges, capability badges, a compact baseline summary, and a restrained “what likely changed?” panel.
- Timeline shows a gap-aware intraday heartrate chart, family filter toggles, real overlay lanes for workouts, enhanced tags, and sessions, selected-event details, and the selected-day event list.
- Trends shows 7d / 30d / 90d windows, baseline-aware trend summaries, sparklines for daily metrics, and confidence notes when history is thin.
- Explain shows the selected day summary, today-vs-baseline or selected-day-vs-baseline framing, supporting evidence bullets, related context entries, and explicit caveats for thin data, missing scopes, or missing measurements.
- Patterns shows deterministic descriptive associations by family and metric with `n`, magnitude, and sufficiency buckets.
- Ops makes trust explicit with auth state, granted scopes, token metadata, last sync per family, database/config paths, the active refresh policy, and record counts for both normalized and derived tables.

Shared interaction semantics:

- Dashboard, Timeline, and Explain all share one selected day
- Timeline and Explain share the same selected event where relevant
- Timeline, Explain, and Patterns share the same family filter toggles for workouts, tags, and sessions

Key navigation defaults:

- `1-6`: Dashboard, Timeline, Trends, Explain, Patterns, Ops
- `[` / `]`: move the shared selected day on Dashboard, Timeline, and Explain
- `,` / `.`: move the selected heartrate point on Timeline
- `j` / `k`: move the selected event on Timeline and Explain
- `w` / `t` / `s`: toggle workouts, tags, and sessions on Timeline, Explain, and Patterns
- `m`: cycle the metric filter on Patterns

## Canonical event model

Context overlays and explainability are powered by a persisted derived read model instead of ad hoc widget assembly.

`derived_context_events` unifies workouts, legacy tags, enhanced tags, and sessions behind normalized fields such as:

- stable derived id
- family
- source id
- anchor day
- start / end timestamps
- point, interval, or all-day semantics
- title
- subtype
- notes
- intensity
- metadata needed for drill-down

This model is rebuilt from persisted normalized data and then queried by the app layer for Timeline, Explain, and Patterns.

## Explainability rules

Explain and Patterns intentionally stay honest and deterministic.

The app does:

- compare the selected day against persisted rolling baselines
- surface nearby workouts, enhanced tags, and sessions as possible context
- show explicit evidence bullets and caveats
- show data sufficiency using sample counts and buckets
- say when a scope or measurement is missing

The app intentionally avoids:

- causal claims
- medical advice
- “AI says” language
- faux significance claims
- certainty theater

Preferred wording includes:

- `associated with`
- `co-occurred with`
- `after days with`
- `this may be relevant, but evidence is limited`

## Pattern engine

The pattern engine is a lightweight descriptive analytics layer. It computes deterministic associations for normalized event keys using persisted history and rolling baselines.

Current surfaced relation windows include:

- same-day activity deltas
- same-day heartrate context
- same-night sleep deltas
- next-day readiness deltas

Current sufficiency rules:

- patterns are hidden until at least `n = 3` comparable occurrences exist
- the UI always shows `n`
- confidence is bucketed by data sufficiency, not by significance claims
- sparse or missing history is surfaced as “not enough data yet”

## Freshness semantics

The UI does not collapse every problem into a generic error. Each data family resolves to a specific state:

- `fresh`: the family has recent persisted data inside its freshness window
- `stale`: data exists, but it is older than the configured freshness policy or the last refresh was partial
- `no data yet`: sync ran, but there are still no persisted rows for that family
- `never synced`: no successful sync has happened yet
- `missing scope`: the required Oura scope was not granted
- `auth failure`: the last failure was due to auth/session problems
- `source delayed`: Oura has not closed out the latest daily family yet, so the app compares against the latest fully available day instead of pretending today's row exists

Default responsive refresh policy:

- personal: every 3600s, stale after 72h
- daily: every 300s, stale after 12h
- heartrate: every 60s, stale after 15m
- workouts: every 600s, stale after 24h
- enhanced tags: every 300s, stale after 12h
- sessions: every 300s, stale after 12h

## What is real today

- `auth login` is real for the implemented scope surface
- `sync once` and `sync watch` are real for:
  - `personal_info`
  - `daily_sleep`
  - `daily_readiness`
  - `daily_activity`
  - `heartrate`
  - `workouts`
  - `enhanced_tags`
  - `sessions`
- `derive rebuild` is real and rebuilds:
  - canonical context events
  - persisted pattern summaries
- demo mode is deterministic and still works without network or credentials
- fixture-backed sync and derivation are real and are used for tests and smoke coverage

Still intentionally deferred:

- webhook delivery and subscription lifecycle
- broader Oura collections beyond the current context-family slice
- packaging, installers, and release automation
- exports, sharing, and reports
- generalized machine learning or medical interpretation

## Architecture summary

The codebase stays in a single crate for now, with narrow module boundaries:

- `src/cli.rs`: CLI parsing and help text
- `src/config.rs`: config loading, XDG paths, runtime defaults
- `src/app.rs`: app state, explicit freshness modeling, screen models, shared selected-day / selected-event semantics, and derived presentation shaping
- `src/tui.rs`: Ratatui event loop, background refresh worker wiring, and snapshot rendering
- `src/components/*`: pure rendering for Dashboard, Timeline, Trends, Explain, Patterns, and Ops
- `src/store/*`: SQLite plan, migrations, typed store queries, and sync-state persistence
- `src/oura/*`: OAuth loopback flow, token lifecycle ownership, typed client interface, and sync orchestration
- `src/refresh.rs`: reusable scheduler core for the TUI and `sync watch`
- `src/derive.rs`: deterministic rebuild of canonical context events and pattern summaries
- `src/insights.rs`: deterministic daily baseline helpers used by Dashboard and Explain

UI components do not perform network calls, token refresh, or database writes. The TUI reads presentation models only.

More detail lives in [docs/ARCHITECTURE.md](/home/ubuntu/ringmaster.rs/docs/ARCHITECTURE.md).

The storage backend choice is documented separately in [docs/decisions/20260408-storage-backend-rusqlite.md](/home/ubuntu/ringmaster.rs/docs/decisions/20260408-storage-backend-rusqlite.md).

## Verification

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3
cargo run -- derive rebuild --demo
cargo run -- demo
```

`cargo run -- sync watch --demo --max-iterations 1` remains a useful scheduler smoke path, and `cargo run -- tui --demo` remains a handy interactive non-network layout check.
