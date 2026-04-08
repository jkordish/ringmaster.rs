# ringmaster.rs

`ringmaster.rs` is a local-first Rust terminal application for exploring Oura Cloud data with a Ratatui interface, SQLite-backed local storage, deterministic demo and fixture paths, a real Oura Cloud API v2 integration, and a near-real-time webhook-aware freshness model where the upstream API supports it.

## Status

This repository now includes an operationally trustworthy personal observability MVP:

- `clap` CLI with `tui`, `tui --demo`, `doctor`, `auth login`, `sync once`, `sync watch`, `derive rebuild`, `webhook serve`, `webhook replay`, `webhook subscriptions list`, `webhook subscriptions sync`, and the compatibility alias `demo`
- a useful Ratatui Dashboard, Timeline, Trends, Explain, Patterns, and Ops screen backed by persisted SQLite data
- deterministic demo data that exercises the same six-screen shell without credentials or network access
- real loopback OAuth login with server-side code exchange, PKCE, and CSRF-safe state handling
- persisted auth/session metadata in SQLite with token secrets stored through the OS keyring seam
- real sync for personal info, daily summaries, heartrate, workouts, enhanced tags, and sessions into normalized tables plus raw payload cache
- persisted derived read models for canonical context events and deterministic pattern summaries, with bounded recent-window refreshes after successful syncs and full-history rebuilds via `derive rebuild`
- a hybrid `sync watch` engine that consumes webhook invalidations first, preserves scheduled fallback reconciliation, and keeps unsupported families honest instead of pretending they are realtime
- a dedicated webhook receiver with explicit verification, accepted/rejected delivery audit, invalidation enqueue, and clean shutdown
- declarative webhook subscription lifecycle management with list, diff, dry-run, create, update, renew, and optional prune flows
- explicit freshness-source and stale-reason semantics instead of a generic “fresh/stale/error” model
- a substantially upgraded Ops and `doctor` surface for receiver health, subscription expiry, queue lag, delivery history, and freshness debugging
- structured logging via `tracing`

The project is intentionally not feature-complete yet. The goal is a trustworthy local foundation with one operationally credible observability slice, not a one-shot full product dump.

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
cargo run -- webhook serve
cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json
cargo run -- webhook subscriptions list --fixture-dir tests/fixtures/webhooks
cargo run -- webhook subscriptions sync --dry-run --fixture-dir tests/fixtures/webhooks
cargo run -- demo
```

Rust toolchain baseline: `rust-version = 1.88`.

Behavior notes:

- `ringmaster tui` launches the live TUI when attached to a terminal. Without a TTY it renders a snapshot of the current local app state instead.
- `ringmaster tui --demo` launches the same UI shell in deterministic demo mode. Without a TTY it renders a text snapshot, which is useful for CI and smoke-oriented workflows.
- `ringmaster demo` remains a compatibility alias for `ringmaster tui --demo`.
- `ringmaster doctor` resolves paths, initializes SQLite, applies migrations, and prints auth, capability, per-family freshness, receiver, subscription, queue, and record-count diagnostics.
- `ringmaster auth login` starts a loopback OAuth flow, validates state, exchanges the code server-side, and persists auth/session metadata locally.
- `ringmaster sync once` refreshes auth when needed, imports all supported sync families, caches raw payloads, upserts normalized SQLite rows, and refreshes the derived context/pattern tables over a bounded recent window when the underlying persisted data changes.
- `ringmaster sync watch` is the long-running invalidation consumer and scheduler. It processes queued webhook invalidations first, then preserves periodic fallback reconciliation.
- `ringmaster webhook serve` is the dedicated HTTP receiver. It verifies Oura webhook requests, records accepted and rejected deliveries, enqueues invalidations, and responds after durable enqueue instead of after sync work.
- `ringmaster webhook replay --fixture tests/fixtures/webhooks/sample.json` is the canonical local debugging path for receiver and queue behavior. It replays a stored HTTP envelope through the same verification, enqueue, and bounded processing path.
- `ringmaster webhook subscriptions list` inspects desired and remote subscription state.
- `ringmaster webhook subscriptions sync --dry-run` prints the convergence plan without mutating remote state. Add `--prune` only when you explicitly want out-of-spec remote subscriptions removed.

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

[webhook]
bind = "127.0.0.1:8789"
path = "/oura/webhook"
public_base_url = "https://your-public-host.example.com"
signature_tolerance_secs = 300
heartbeat_secs = 15
renewal_lead_secs = 86400

[[webhook.subscriptions]]
data_type = "daily_sleep"
event_types = ["create", "update", "delete"]

[[webhook.subscriptions]]
data_type = "daily_readiness"
event_types = ["create", "update", "delete"]

[[webhook.subscriptions]]
data_type = "daily_activity"
event_types = ["create", "update", "delete"]

[[webhook.subscriptions]]
data_type = "workout"
event_types = ["create", "update", "delete"]

[[webhook.subscriptions]]
data_type = "enhanced_tag"
event_types = ["create", "update", "delete"]

[[webhook.subscriptions]]
data_type = "session"
event_types = ["create", "update", "delete"]
```

Set secrets in the environment instead of the config file:

```bash
export RINGMASTER_OURA_CLIENT_SECRET="your-oura-client-secret"
export RINGMASTER_WEBHOOK_VERIFICATION_TOKEN="your-oura-webhook-verification-token"
```

Important notes:

- granted scopes are not configured in `config.toml`; they come from the persisted auth session
- token secrets are intentionally kept out of plaintext config
- the receiver is local-first only; there is no hosted relay service
- a real Oura webhook deployment requires a user-managed public HTTPS endpoint, reverse proxy, or tunnel that forwards to `webhook.bind`

## Supported families

The current live sync and persistence surface includes:

- `personal`
- `daily`
- `heartrate`
- `workout`
- `enhanced_tag`
- `session`

Legacy `tags` remain read-compatible in the derived event layer when they already exist in the database, but the product is intentionally centered on `enhanced_tag` for current work.

Webhook-driven freshness is intentionally limited to Oura `data_type`s the product actually supports:

- `daily_sleep`
- `daily_readiness`
- `daily_activity`
- `workout`
- `enhanced_tag`
- `session`

`heartrate` remains scheduled-only because Oura does not currently expose it as a webhook `data_type`.

## Product behavior

What the screens now do:

- Dashboard shows the shared selected day, daily metric cards, freshness badges, capability badges, a compact baseline summary, and a restrained “what likely changed?” panel.
- Timeline shows a gap-aware intraday heartrate chart, family filter toggles, real overlay lanes for workouts, enhanced tags, and sessions, selected-event details, and the selected-day event list.
- Trends shows 7d / 30d / 90d windows, baseline-aware trend summaries, and confidence notes when history is thin.
- Explain shows the selected day summary, today-vs-baseline or selected-day-vs-baseline framing, supporting evidence bullets, related context entries, and explicit caveats for thin data, missing scopes, or missing measurements.
- Patterns shows deterministic descriptive associations by family and metric with `n`, magnitude, and sufficiency buckets.
- Ops acts as a local operator console with auth state, granted scopes, per-family freshness source, webhook receiver status, callback configuration, subscription drift and expiry, queue depth and lag, delivery history, and current runtime mode.

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
- say when a scope, delivery path, or measurement is missing

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

## Freshness semantics

The UI no longer flattens every problem into generic stale/error state. Each data family resolves to a specific state:

- `fresh via webhook`
- `fresh via periodic reconcile`
- `stale: no recent delivery`
- `stale: sync failed`
- `stale: webhook unsupported`
- `stale: receiver down`
- `stale: subscription missing or expired`
- `stale: capability missing`
- `stale: upstream data pending`

This is the core “trustworthiness” upgrade in phase 4: the product tells you why freshness is good or bad instead of forcing you to infer it.

## Replay and operations workflow

The preferred local debugging path is:

```bash
cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json
```

The preferred local subscription smoke path is:

```bash
cargo run -- webhook subscriptions sync --dry-run --fixture-dir tests/fixtures/webhooks
```

The preferred bounded watch smoke path is:

```bash
cargo run -- sync watch --demo --max-iterations 1
```

Together, those commands let you exercise receiver, queue, scheduler, and subscription logic without waiting for a real Oura webhook delivery.
