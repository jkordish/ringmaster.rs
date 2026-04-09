# ringmaster.rs

`ringmaster.rs` is a local-first Rust terminal application for exploring Oura Cloud data with a Ratatui interface, SQLite-backed local storage, deterministic demo and fixture paths, a real Oura Cloud API v2 integration, a near-real-time webhook-aware freshness model where the upstream API supports it, a deterministic review layer for daily briefs, weekly drift, and bounded investigations, and a centralized visual design system with scenario-rich snapshot QA for consistent screen choreography.

## Status

This repository now includes an operationally trustworthy personal observability MVP with a deliberate visual system:

- `clap` CLI with `tui`, `tui --demo`, `ui snapshot`, `doctor`, `auth login`, `sync once`, `sync watch`, `derive rebuild`, `review today`, `review week`, `review investigate`, `webhook serve`, `webhook replay`, `webhook subscriptions list`, `webhook subscriptions sync`, and the compatibility alias `demo`
- a useful Ratatui Dashboard, Timeline, Trends, Explain, Patterns, Review, and Status screen backed by persisted SQLite data
- deterministic demo data that exercises the same seven-screen shell without credentials or network access
- a centralized semantic theme/token layer for palette roles, spacing rhythm, badge language, panel chrome, breakpoint-aware layout, and chart styling
- stronger screen-specific reading paths so Dashboard, Timeline, Trends, Explain, Patterns, Review, and Status no longer feel like the same grid with different labels
- deterministic visual QA via `ringmaster ui snapshot` for demo, single-fixture, and canonical phase-7 scenario-matrix snapshot generation across compact, medium, and wide terminal sizes
- real loopback OAuth login with server-side code exchange, PKCE, and CSRF-safe state handling
- persisted auth/session metadata in SQLite with token secrets stored through the OS keyring seam
- real sync for personal info, daily summaries, heartrate, workouts, enhanced tags, sessions, daily stress, daily resilience, sleep time, cardiovascular age, VO2 max, and rest mode periods into normalized tables plus raw payload cache
- persisted derived read models for canonical context events, deterministic pattern summaries, and review signal snapshots, with bounded recent-window refreshes after successful syncs and full-history rebuilds via `derive rebuild`
- deterministic daily and weekly review ranking plus bounded investigations with explicit evidence, counterevidence, confidence, and sufficiency
- a deliberately non-chatty smart layer: no freeform assistant, no hosted AI service, and no hidden text generation
- a hybrid `sync watch` engine that consumes webhook invalidations first, preserves scheduled fallback reconciliation, and keeps unsupported families honest instead of pretending they are realtime
- a dedicated webhook receiver with explicit verification, accepted/rejected delivery audit, invalidation enqueue, and clean shutdown
- declarative webhook subscription lifecycle management with list, diff, dry-run, create, update, renew, and optional prune flows
- explicit freshness-source and stale-reason semantics instead of a generic “fresh/stale/error” model
- a substantially upgraded Status and `doctor` surface for receiver health, subscription expiry, queue lag, delivery history, and freshness debugging
- structured logging via `tracing`

The project is intentionally not feature-complete yet. The goal is a trustworthy local foundation with one operationally credible observability slice, not a one-shot full product dump.

## Commands

```bash
cargo run -- tui
cargo run -- tui --demo
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots
cargo run -- doctor
cargo run -- auth login
cargo run -- sync once
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3
cargo run -- sync watch
cargo run -- sync watch --demo --max-iterations 1
cargo run -- derive rebuild
cargo run -- derive rebuild --demo
cargo run -- review today --demo
cargo run -- review week --demo
cargo run -- review investigate --focus readiness --demo
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
- `ringmaster ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots` remains the canonical non-interactive design QA smoke path. It writes deterministic text artifacts for selected screens and sizes and reuses the same rendering stack as the interactive UI.
- `ringmaster ui snapshot --fixture-dir tests/fixtures/phase7 ...` enables the deeper phase-7 scenario matrix. It expands the same screen and size selection across `strong`, `weak`, `empty`, `stale`, and `error` states and writes scenario-tagged artifacts.
- `ringmaster demo` remains a compatibility alias for `ringmaster tui --demo`.
- `ringmaster doctor` resolves paths, initializes SQLite, applies migrations, and prints auth, capability, per-family freshness, receiver, subscription, queue, and record-count diagnostics.
- `ringmaster auth login` starts a loopback OAuth flow, validates state, exchanges the code server-side, and persists auth/session metadata locally.
- `ringmaster sync once` refreshes auth when needed, imports all supported sync families, caches raw payloads, upserts normalized SQLite rows, and refreshes the derived context/pattern tables over a bounded recent window when the underlying persisted data changes.
- `ringmaster sync watch` is the long-running invalidation consumer and scheduler. It processes queued webhook invalidations first, then preserves periodic fallback reconciliation.
- `ringmaster review today` prints a ranked daily brief with evidence, uncertainty, confidence, and sufficiency labels.
- `ringmaster review week` prints a ranked weekly review using the same persisted local data and deterministic templates.
- `ringmaster review investigate --focus <readiness|sleep|recovery|stress|activity>` prints a bounded investigation with evidence bundles, counterevidence, and “look next” pointers.
- `ringmaster webhook serve` is the dedicated HTTP receiver. It verifies Oura webhook requests, records accepted and rejected deliveries, enqueues invalidations, and responds after durable enqueue instead of after sync work.
- `ringmaster webhook replay --fixture tests/fixtures/webhooks/sample.json` is the canonical local debugging path for receiver and queue behavior. Fixture replay runs the same verification and enqueue path offline, then previews the bounded invalidation-processing plan without writing fixture data into the local store. Stored-delivery replay re-enqueues invalidations without auto-running a fixture-backed sync into a live store.
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

## Visual system

Phase 6 turns the interface into a more deliberate observability instrument instead of a collection of similarly styled panels.

Implemented visual principles:

- at-a-glance comprehension comes first, with the most important thing on each screen given the strongest positional and typographic emphasis
- palette decisions are semantic and centralized instead of scattered through widgets
- states never rely on color alone; badges, wording, prefixes, ordering, and chrome all contribute
- compact, medium, and wide terminals use different layout strategies instead of one compressed layout trying to fit everywhere
- charts and compact displays prefer familiar forms: lines for time, bars for comparison, sparklines for directional hints

The canonical references are:

- `docs/DESIGN_AUDIT.md`
- `docs/DESIGN_SYSTEM.md`

## Supported families

The current live sync and persistence surface includes:

- `personal`
- `daily`
- `heartrate`
- `workout`
- `enhanced_tag`
- `session`
- `daily_stress`
- `daily_resilience`
- `sleep_time`
- `daily_cardiovascular_age`
- `vo2_max`
- `rest_mode_period`

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

- Dashboard is the editorial front page. It leads with “what matters now,” follows with a daily metric band and freshness/capability framing, and uses drill-down cues as the tertiary rhythm.
- Timeline is the immersive temporal view. The chart is first, overlay lanes are second, and selected details plus day events come last.
- Trends is the comparative scanning surface. It emphasizes windows, deltas, baselines, and compact directional hints in a matrix-like rhythm.
- Explain is the evidence view. It narrows focus to a selected day, presents the primary claim first, then measured inputs, evidence bundles, context, and uncertainty.
- Timeline, Explain, and Review now expose lightweight breadcrumbs when they materially reduce cognitive load. These breadcrumbs keep the current day, linked event, and carryover context visible without changing the render pipeline.
- Patterns is the grouped association browser. It clusters findings by interpretation and comparison instead of reading like another evidence detail page.
- Review is the editorial briefing surface. It presents ranked observations, concise rationale, and bounded investigations without becoming another dashboard clone.
- Status remains the utilitarian operator console, but with clearer grouping, stronger status emphasis, and less visual competition between diagnostics.

Shared interaction semantics:

- Dashboard, Timeline, Explain, and Review all share one selected day
- Timeline and Explain share the same selected event where relevant
- Timeline, Explain, and Patterns share the same family filter toggles for workouts, tags, and sessions
- when fresh data replaces the live snapshot, the shared selected day stays anchored to the exact day if it still exists, otherwise the nearest earlier available day, then the next later day, before finally falling back to the newest day

Key navigation defaults:

- `1-7`: Dashboard, Timeline, Trends, Explain, Patterns, Review, Status
- `[` / `]`: move the shared selected day on Dashboard, Timeline, Explain, and Review
- `,` / `.`: move the selected heartrate point on Timeline
- `j` / `k`: move the selected event on Timeline and Explain
- `j` / `k` on Review: move the selected review card
- `w` / `t` / `s`: toggle workouts, tags, and sessions on Timeline, Explain, and Patterns
- `m`: cycle the metric filter on Patterns
- `v`: cycle Today, Week, and Investigate on Review
- `f`: cycle investigation focus on Review

## Visual QA

Use `ringmaster ui snapshot` for deterministic non-interactive review and regression testing.

Examples:

```bash
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots
cargo run -- ui snapshot --demo \
  --screen dashboard --screen timeline --screen review --screen status \
  --size compact --size wide \
  --out-dir /tmp/ringmaster-ui-snapshots-smoke
cargo run -- ui snapshot \
  --fixture-dir tests/fixtures/phase7 \
  --screen dashboard --screen explain --screen review --screen status \
  --size compact --size wide \
  --out-dir /tmp/ringmaster-ui-snapshots-phase7-smoke
```

Current deterministic sizes:

- `compact`: `90x28`
- `medium`: `120x36`
- `wide`: `160x44`

Phase-7 scenario meanings:

- `strong`: healthy local cache, full capability coverage, rich context, and current freshness
- `weak`: sparse but still usable local history with explicit uncertainty
- `empty`: scopes are granted, but the local cache has not accumulated records yet
- `stale`: persisted data exists, but sync age or webhook/runtime state has drifted outside the expected freshness window
- `error`: capability, auth, or sync failure states that block part of the product surface

Artifact naming:

- demo and legacy single-fixture mode keep `screen-size.txt`
- phase-7 scenario-matrix mode writes `screen-scenario-size.txt`

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

## Smart review model

The smart layer is deterministic and capability-aware. It is intentionally not a chat assistant.

- Reviewable signals are defined in a canonical registry with metadata for baseline window, directionality, evidence kind, safe wording, and surface suitability.
- `derive rebuild` persists `derived_review_signal_days`, which are normalized per-signal daily feature snapshots rebuilt from local stored data.
- `review today` ranks anchor-day observations using deviation from baseline, persistence, recency, corroboration, counterevidence penalties, freshness penalties, and data-sufficiency penalties.
- `review week` ranks weekly drift using a 7-day anchor window against a prior 28-day comparison window.
- `review investigate` is bounded to fixed focuses: readiness, sleep, recovery, stress, and activity.
- Every observation is rendered from deterministic templates with:
  - a headline
  - a “why this is shown” explanation
  - evidence
  - counterevidence or uncertainty
  - confidence
  - sufficiency

The review layer avoids:

- freeform prompts
- causal claims
- medical interpretation
- “AI says” wording
- certainty theater

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
- chat-assistant framing

Preferred wording includes:

- `associated with`
- `co-occurred with`
- `below your baseline`
- `evidence is limited because...`
- `carryover from <day>`
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
