# IMPLEMENT.md

## Purpose

This file is the execution runbook for the current phase-3 product. It should only describe flows that work today.

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
7. Updates per-family sync watermarks, status, failure counts, backoff state, and last structured errors

Fixture-backed bounded equivalent:

```bash
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3
```

That command uses the same importer path without live credentials and without mutating SQLite.
Because it is a dry-run path, it does not rebuild the derived SQLite tables.

### `sync watch`

`ringmaster sync watch` runs the reusable scheduler without the TUI. It:

1. Reads persisted sync state
2. Computes due families from the configured refresh policy and any persisted backoff
3. Reuses the same `sync_selected(...)` engine as `sync once`
4. Sleeps until the next due family, unless a bounded run is requested
5. Exits cleanly after `--max-iterations N` when used in CI/debug workflows

The bounded demo smoke path is:

```bash
cargo run -- sync watch --demo --max-iterations 1
```

That command uses the checked-in fixtures by default, does not require live credentials, and is the preferred scheduler smoke test.

## Derived rebuild flow

### `derive rebuild`

`ringmaster derive rebuild` is the non-network rebuild workflow for derived product state. It:

1. Opens the existing SQLite database
2. Reads persisted normalized workouts, tags, enhanced tags, sessions, and daily history
3. Rebuilds canonical context events
4. Rebuilds persisted pattern summaries
5. Replaces the derived tables through typed store APIs
6. Prints the number of rebuilt context events and pattern summaries

This is the explicit full-history recompute path. The auto-refresh that runs after normal syncs uses a bounded recent window so repeated background refreshes stay responsive as the local database grows.

Demo rebuild path:

```bash
cargo run -- derive rebuild --demo
```

That command seeds a temporary store from `tests/fixtures/phase3`, runs the same derivation logic, prints a summary, and exits cleanly.

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

- shows auth/session state
- shows granted capabilities
- shows per-family freshness and sync diagnostics
- shows record counts for normalized and derived tables

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

Each data family resolves to an explicit state:

- `fresh`
- `stale`
- `no data yet`
- `never synced`
- `missing scope`
- `auth failure`
- `source delayed`

These are derived from persisted sync state, granted scopes, auth/session diagnostics, and the configured freshness policy. The app intentionally does not collapse them into a generic error label.

Default responsive policy:

- personal: refresh every `3600s`, stale after `72h`
- daily: refresh every `300s`, stale after `12h`
- heartrate: refresh every `60s`, stale after `15m`
- workouts: refresh every `600s`, stale after `24h`
- enhanced tags: refresh every `300s`, stale after `12h`
- sessions: refresh every `300s`, stale after `12h`

## Explainability and pattern wording rules

Explain and Patterns are deterministic presentation layers, not language-model features.

The product should say:

- `associated with`
- `co-occurred with`
- `after days with`
- `this may be relevant, but evidence is limited`

The product should not say:

- `caused by`
- medical advice
- significance claims
- freeform AI narrative language

## Fixture and demo behavior

- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3` runs the same parse/normalize pipeline without live credentials and without mutating SQLite
- `cargo run -- sync watch --demo --max-iterations 1` runs the same scheduler/import path in bounded fixture mode
- `cargo run -- derive rebuild --demo` seeds a temporary store from the same phase-3 fixtures and rebuilds the derived tables
- `cargo run -- tui --demo` uses deterministic in-memory presentation data and skips live background refresh
- `cargo run -- demo` is an alias for `cargo run -- tui --demo`

## Doctor expectations

`cargo run -- doctor` now reports:

- resolved config/state/cache/database paths
- auth/session state and token timing metadata
- granted capabilities
- per-family sync state including failure counts and next-attempt backoff
- the active refresh policy for all six families
- record counts for normalized and derived tables
- the default demo fixture directory

## Verification sequence

Use this order unless a narrower check is sufficient while developing:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3
cargo run -- derive rebuild --demo
cargo run -- demo
```

Additional smoke checks worth keeping in mind:

- `cargo run -- sync watch --demo --max-iterations 1`
- interactive `cargo run -- tui --demo`
- live screen assertions via the Ratatui `TestBackend`

## Notes for future passes

- Keep UI rendering pure; any new sync/auth work belongs outside `src/components/*`.
- Reuse the scheduler and derive seams instead of inventing separate watch/TUI/rebuild logic.
- Webhook invalidation can plug into the current scheduler later, but webhook infrastructure is still intentionally deferred.
