# IMPLEMENT.md

## Purpose

This file is the execution runbook for the current phase-1 foundation. It should only describe flows that work today.

## Commands

Current commands:

```bash
cargo run -- tui
cargo run -- demo
cargo run -- doctor
cargo run -- auth login
cargo run -- sync once
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1
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

## Sync flow

`ringmaster sync once` is the real phase-1 vertical slice. It:

1. Inspects persisted auth/session state
2. Refreshes tokens when needed through the auth layer
3. Fetches the phase-1 slice:
   - personal info
   - daily sleep
   - daily readiness
   - daily activity
   - heartrate
4. Caches raw payloads separately from normalized tables
5. Performs idempotent upserts into SQLite
6. Updates per-slice sync watermarks, status, and last structured errors

## Fixture and dry-run behavior

- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1`
  runs the same parse/normalize pipeline without live credentials and without mutating SQLite
- fixture mode intentionally uses a broad history window so the checked-in fixtures stay stable in CI over time
- demo mode remains deterministic and is independent from live auth/sync

## UI expectations

- Dashboard reads persisted daily rows and shows missing-capability or empty states honestly
- Timeline and Trends stay explicit when heartrate scope or data is missing
- Ops shows auth state, granted scopes, token metadata, last sync times/errors, database path, and config path
- widgets remain pure renderers; they never trigger network or storage side effects

## Verification sequence

Use this order unless a narrower check is sufficient while developing:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1
```

Additional smoke checks worth keeping in mind:

- non-interactive `cargo run -- demo`
- live UI screen tests via the Ratatui `TestBackend`

## Notes for future passes

- Do not treat config as the source of truth for granted scopes or token freshness.
- Keep UI rendering pure; any new sync/auth work belongs outside `src/components/*`.
- Extend the existing real slice before broadening the Oura surface.
