# IMPLEMENT.md

## Purpose

This file is the execution runbook for the current phase-4 product. It only describes flows that work today.

## Commands

Current commands:

```bash
cargo run -- tui
cargo run -- tui --demo
cargo run -- demo
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
```

## Auth flow

`ringmaster auth login` performs the live Oura server-side OAuth flow:

1. Load `client_id` from config or env and `client_secret` from `RINGMASTER_OURA_CLIENT_SECRET`
2. Start a one-shot loopback listener on the configured callback bind/path
3. Print the authorization URL
4. Validate the returned `state` value and capture granted scopes
5. Exchange the authorization code server-side with PKCE
6. Persist non-secret auth/session metadata in SQLite
7. Persist access/refresh tokens through the keyring-backed secret store seam

Denied auth and partial scopes are preserved as explicit local state instead of being silently treated as success.

## Sync flows

### `sync once`

`ringmaster sync once` is the one-shot importer. It:

1. Inspects persisted auth/session state
2. Refreshes tokens when needed through the auth layer
3. Fetches the supported live families:
   - personal info
   - daily sleep
   - daily readiness
   - daily activity
   - heartrate
   - workouts
   - enhanced tags
   - sessions
4. Caches raw payloads separately from normalized tables
5. Performs idempotent upserts into SQLite
6. Rebuilds the derived context-event and pattern-summary tables over a bounded recent window when any daily or context-family data was updated
7. Updates per-family sync watermarks, status, failure counts, backoff state, last structured errors, and trigger provenance

Fixture-backed bounded equivalent:

```bash
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3
```

That command uses the same importer path without live credentials and without mutating SQLite.
Because it is a dry-run path, it does not rebuild the derived SQLite tables.

### `sync watch`

`ringmaster sync watch` is now the hybrid invalidation consumer and scheduler. It:

1. Reads persisted sync state and webhook queue state
2. Writes a runtime heartbeat so Ops and `doctor` can distinguish active watch mode from a down worker
3. Claims pending webhook invalidations first
4. Coalesces invalidations by family and runs targeted `sync_selected(...)` windows with webhook trigger provenance
5. Applies delete-side effects for supported context families when Oura sends delete events
6. Records processing attempts, failures, retry state, and completion timestamps
7. Preserves scheduled fallback reconciliation for all families
8. Exits cleanly after `--max-iterations N` when used in CI/debug workflows

The bounded demo smoke path is:

```bash
cargo run -- sync watch --demo --max-iterations 1
```

That command uses the checked-in fixtures by default, does not require live credentials, and is the preferred scheduler smoke test.

### Trigger provenance

Successful sync slices now persist why they ran:

- `manual_sync`
- `periodic_reconcile`
- `webhook_invalidation`

This provenance is used by the app, Ops, and `doctor` so freshness can be explained instead of guessed.

## Webhook receiver flow

### `webhook serve`

`ringmaster webhook serve` is the dedicated HTTP receiver. It:

1. Loads receiver config from `[webhook]`
2. Binds an Axum HTTP server on the configured address
3. Exposes the configured webhook path plus lightweight `/healthz` and `/readyz` routes
4. Handles Oura verification challenge requests on the webhook path
5. Verifies POST signatures and timestamp freshness explicitly
6. Persists accepted deliveries into the raw delivery audit log
7. Persists rejected deliveries with explicit reason codes
8. Enqueues derived invalidations after durable acceptance
9. Responds immediately after durable enqueue instead of waiting for sync work
10. Writes receiver heartbeats and shuts down cleanly on Ctrl-C

Important operational boundary:

- `webhook serve` never performs sync work inline
- `sync watch` is the only long-running invalidation consumer

### Security and verification behavior

The receiver currently enforces:

- Oura verification challenge handling for GET requests
- explicit HMAC-SHA256 signature verification for POST requests
- explicit timestamp validation with configurable tolerance
- duplicate delivery dedupe
- sanitized header persistence so secrets are not leaked into audit metadata
- inspectable rejection reasons instead of generic 500s for malformed or unauthorized requests

## Subscription lifecycle flow

### Desired subscriptions

Desired webhook subscriptions are declared locally in config through `[[webhook.subscriptions]]`. Each entry includes:

- `data_type`
- `event_types`
- optional `enabled`

The runtime also has a default desired set aligned with the currently supported webhook families.

### `webhook subscriptions list`

`ringmaster webhook subscriptions list` reads the current desired config and resolves remote subscription state from either:

- the live Oura admin API, or
- fixture-backed JSON when `--fixture-dir` is used

It prints an inspectable report including remote expiry and drift state.

### `webhook subscriptions sync`

`ringmaster webhook subscriptions sync` is the declarative convergence command. It:

1. Loads desired subscription specs from config
2. Fetches the current remote subscription snapshot
3. Computes a plan that may create, update, renew, or optionally prune remote subscriptions
4. Persists desired and remote snapshot metadata locally
5. Executes the plan unless `--dry-run` is set
6. Reports explicit diffs and renewal horizon information

Safe default:

```bash
cargo run -- webhook subscriptions sync --dry-run --fixture-dir tests/fixtures/webhooks
```

Pruning is never implicit. Unexpected remote subscriptions are only deleted when `--prune` is explicitly supplied.

## Replay and local debugging

### `webhook replay`

`ringmaster webhook replay` is the canonical local debugging path. It can replay:

- fixture-backed deliveries from disk
- one stored accepted delivery by id
- a recent range of accepted deliveries

Fixture replay accepts stored request envelopes with method, query, headers, and body so the same verification code path can run offline.

Canonical smoke path:

```bash
cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json
```

That fixture exercises:

- verification challenge or signed-delivery parsing rules
- signature/timestamp verification
- accepted-delivery persistence
- invalidation enqueue
- one bounded invalidation-processing pass through the same watch-side logic

## Derived rebuild flow

### `derive rebuild`

`ringmaster derive rebuild` is still the explicit non-network rebuild workflow for derived product state. It:

1. Opens the existing SQLite database
2. Reads persisted normalized workouts, tags, enhanced tags, sessions, and daily history
3. Rebuilds canonical context events
4. Rebuilds persisted pattern summaries
5. Replaces the derived tables through typed store APIs
6. Prints the number of rebuilt context events and pattern summaries

This is the explicit full-history recompute path. Normal syncs still use bounded recent-window rebuilds so repeated background refreshes stay responsive as the database grows.

## TUI runtime

`ringmaster tui` is the live product path. It:

1. Opens the store and reads auth/session metadata
2. Builds an initial `LiveSnapshot`
3. Starts the Ratatui event loop
4. Starts a dedicated background refresh worker on a separate thread
5. Reuses the scheduler core plus `sync_selected(...)` inside that worker
6. Rebuilds a fresh `LiveSnapshot` after each successful refresh
7. Sends snapshot updates back into the reducer as actions

Important boundary:

- widgets never perform HTTP
- widgets never refresh tokens
- widgets never write to SQLite
- the render path stays on persisted presentation models only

## Screen behavior

### Dashboard

- shows the shared selected day
- shows daily metric cards and baseline framing
- shows freshness and capability banners
- shows a restrained “what likely changed?” summary

### Timeline

- shows a gap-aware intraday heartrate chart
- overlays workouts, enhanced tags, and sessions in separate lanes
- supports family toggles that do not rely on color alone
- shows selected-event details and a selected-day event list

### Trends

- shows 7d / 30d / 90d windows
- shows baseline-aware summaries and thin-history notes

### Explain

- shows the selected-day summary
- compares the selected day against rolling baselines
- shows evidence bullets and related context entries
- shows caveats for thin data, missing scope, or missing measurement coverage

### Patterns

- shows descriptive associations by family and metric
- shows `n`, magnitude, and sufficiency bucket
- explicitly says when there is not enough data yet

### Ops

Ops is now a real operator console for the local-first runtime. It shows:

- auth/session state
- granted capabilities
- per-family freshness and trigger-source diagnostics
- receiver configuration and callback URL
- receiver/watch heartbeats
- desired vs remote subscription summary
- renewal horizon and remote expiry status
- last accepted and last rejected delivery
- queue depth and lag
- last webhook-triggered sync vs periodic reconcile
- record counts for normalized and derived tables
- recent incident summaries

## Shared interaction semantics

Shared state is now intentional rather than screen-specific:

- Dashboard, Timeline, and Explain share one selected day
- Timeline and Explain share one selected event
- Timeline, Explain, and Patterns share family filter toggles for workouts, tags, and sessions

Default key flow:

- `1-6`: Dashboard, Timeline, Trends, Explain, Patterns, Ops
- `[` / `]`: previous/next selected day on Dashboard, Timeline, and Explain
- `,` / `.`: previous/next heartrate point on Timeline
- `j` / `k`: previous/next selected event on Timeline and Explain
- `w` / `t` / `s`: toggle workouts, tags, and sessions on Timeline, Explain, and Patterns
- `m`: cycle the metric filter on Patterns

## Freshness and missing-data semantics

Each data family now resolves to an explicit reasoned state, not a generic stale bucket:

- `fresh via webhook`
- `fresh via periodic reconcile`
- `stale: no recent delivery`
- `stale: sync failed`
- `stale: webhook unsupported`
- `stale: receiver down`
- `stale: subscription missing or expired`
- `stale: capability missing`
- `stale: upstream data pending`

These states are derived from persisted sync state, granted scopes, auth/session diagnostics, receiver and watch heartbeats, remote subscription snapshots, recent delivery history, invalidation queue state, and the configured freshness policy.

## Fixture and demo behavior

- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3` runs the same parse/normalize pipeline without live credentials and without mutating SQLite
- `cargo run -- sync watch --demo --max-iterations 1` runs the same hybrid scheduler/import path in bounded fixture mode
- `cargo run -- derive rebuild --demo` seeds a temporary store from the same phase-3 fixtures and rebuilds the derived tables
- `cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json` runs the webhook receive, verify, enqueue, and bounded processing path offline
- `cargo run -- tui --demo` uses deterministic in-memory presentation data and skips live background refresh
- `cargo run -- demo` is an alias for `cargo run -- tui --demo`

## Doctor expectations

`cargo run -- doctor` now reports:

- resolved config/state/cache/database paths
- auth/session state and token timing metadata
- granted capabilities
- per-family sync state including failure counts and next-attempt backoff
- the active refresh policy for all six families
- receiver configuration readiness and callback URL
- verification-token and public-endpoint readiness
- receiver and watch heartbeat state
- runtime mode
- desired and remote subscription counts
- remote subscription health and renewal needs
- last accepted and rejected delivery
- queue depth, queue age, and failed processing attempts
- record counts for normalized and derived tables
- the default demo fixture directory

## Verification sequence

Use this order unless a narrower check is sufficient while developing:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json
cargo run -- sync watch --demo --max-iterations 1
cargo run -- webhook subscriptions sync --dry-run --fixture-dir tests/fixtures/webhooks
```

Additional smoke checks worth keeping in mind:

- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3`
- `cargo run -- derive rebuild --demo`
- interactive `cargo run -- tui --demo`
- interactive `cargo run -- webhook serve` behind a user-managed public HTTPS callback

## Notes for future passes

- Keep UI rendering pure; any new sync/auth/webhook work belongs outside `src/components/*`.
- Reuse the scheduler, queue, and derive seams instead of inventing separate receiver/watch/TUI logic.
- Do not claim real-time coverage for families Oura does not expose as webhook `data_type`s.
- Hosted relay services and tunnel orchestration remain intentionally deferred.
