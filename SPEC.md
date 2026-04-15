# SPEC.md

# ringmaster.rs

## One-line product definition

`ringmaster.rs` is a local-first Rust terminal observability app for Oura Cloud data, built with Ratatui, focused on fresh data, trend analysis, and useful interpretation instead of raw API dumping.

## Product stance

This project is not a generic dashboard and not a cloud SaaS. It is a personal, privacy-conscious terminal tool that:

- pulls a user's Oura data through Oura Cloud API v2
- caches and normalizes data locally
- renders a fast, keyboard-first TUI
- explains changes, freshness, and trends
- stays useful even when network access is unavailable by using cached data and demo fixtures

## Goals

1. **Local-first**
   - all persistent data lives on the user's machine by default
   - credentials and secrets use local secure storage seams
   - the app remains useful from cache without live API calls

2. **Explain, not just display**
   - show today's scores and data freshness
   - surface 7/30/90-day baselines
   - highlight what changed and what may have contributed

3. **Strong foundations**
   - compileable, testable, typed Rust
   - clean separation of UI, sync, and storage
   - deterministic demo mode for development and screenshots

4. **Production-sane ergonomics**
   - keyboard-first TUI
   - consistent CLI
   - logging, doctor command, and explicit error messages

## Non-goals for bootstrap and v1

- multi-user cloud service
- web frontend
- mobile client
- public webhook infrastructure as a hard requirement
- medical diagnosis or clinical claims
- complicated plugin system
- premature workspace split into many crates

## Target users

Primary user: the repository owner, a technically strong user who wants a terminal-native, inspectable, scriptable tool rather than another glossy health app.

Secondary user: other advanced Oura users who are comfortable with local CLI tools.

## Oura integration stance

### Required truths

- Use **Oura Cloud API v2**.
- Use **OAuth2** for user data access.
- Treat **webhooks as optional but recommended**, not required for the first useful version.
- Design for **poll-first bootstrap**, with webhook hooks left ready for later.
- Assume different scopes may be granted independently.
- Handle partial data gracefully.

### Practical implications

- The first working release must be able to authorize, sync, cache, and render useful data without webhooks.
- Subscription management for webhook endpoints should be isolated behind interfaces and docs so it can land later without refactoring the whole app.
- The app must not assume live streaming from the ring. It should model freshness based on cloud sync behavior.

## Functional requirements

### CLI

The binary name is `ringmaster`.

Initial command surface:

- `ringmaster tui`
  - launch the TUI
- `ringmaster doctor`
  - print environment, config paths, storage paths, and health checks
- `ringmaster auth login`
  - begin OAuth login flow or print next steps if only scaffolded
- `ringmaster sync once`
  - run one sync cycle
- `ringmaster demo`
  - launch deterministic demo mode without Oura credentials

These commands may expand later, but this set is the bootstrap baseline.

### TUI screens

At minimum, the TUI architecture must support these screens, even if initial content is placeholder or demo-backed:

1. **Dashboard**
   - today's sleep/readiness/activity summary
   - freshness badge
   - granted-scope or capability indicator
   - short “what changed” summary

2. **Timeline**
   - intraday series view, starting with heart rate once available
   - placeholder overlays for tags, workouts, and sessions

3. **Trends**
   - 7/30/90-day summaries
   - baseline deltas
   - simple trend summaries

4. **Ops**
   - auth state
   - config paths
   - database path
   - last sync state
   - latest errors and warnings

### Demo mode

Demo mode is mandatory for bootstrap.

Requirements:

- no network access required
- deterministic sample data
- enough variation to exercise dashboard, trends, and ops screens
- usable in CI and screenshots

### Storage

Use a local SQLite database.

Bootstrap schema must account for:

- app metadata and schema version
- sync state
- raw payload cache
- daily summary families
- heart rate time series
- workouts
- tags / enhanced tags
- sessions
- future webhook metadata

Exact schema may begin as skeleton tables, but the migration system must exist early.

### Sync behavior

Bootstrap and v1 sync flow:

1. user authorizes via OAuth
2. app performs initial historical pull
3. app stores raw and normalized records
4. app performs explicit or scheduled polling for updates
5. UI reads from local store, not directly from the network

### Freshness

Every screen that shows health data should expose freshness or staleness.

Examples:
- “synced 4m ago”
- “sleep data stale: last full sync yesterday”
- “scope missing: heart rate”

### Error handling

Errors must be actionable and categorized.

At minimum, distinguish:
- config errors
- auth errors
- transport errors
- API errors
- storage errors
- unsupported or missing-scope errors

## Architecture

## Repo shape

Single-package crate for bootstrap, with these module areas:

- `src/cli.rs`
- `src/config.rs`
- `src/error.rs`
- `src/action.rs`
- `src/app.rs`
- `src/tui.rs`
- `src/components/*`
- `src/oura/*`
- `src/store/*`

## App data flow

```text
CLI / TUI input
      ↓
   App state
      ↓
  render actions
      ↓
  Ratatui components

sync requests
      ↓
 Oura client / auth
      ↓
 storage layer
      ↓
  app reads typed view models
```

## Key boundaries

- UI components are pure consumers of state.
- Sync orchestrates API calls and persistence.
- Storage is the source of truth for rendered data.
- Configuration and secrets are centralized, not ad hoc.
- Demo mode uses the same state/view paths as real data wherever possible.

## Bootstrap milestone plan

### M0: repo seed
- governance docs
- spec
- AGENTS guidance
- compileable std-only skeleton or minimal starter

### M1: runnable app shell
- real CLI
- TUI event loop
- demo mode
- doctor command
- logging and errors

### M2: storage and data seams
- migrations
- typed store layer
- raw payload cache
- sync state

### M3: Oura auth and first sync
- OAuth loopback scaffold or implementation
- personal info and daily summaries
- local cache-backed dashboard

### M4: richer views
- heart rate timeline
- trends
- overlays for workouts, tags, and sessions

### M5: interpretation
- baseline comparisons
- freshness indicators
- “what changed” summaries

### M6: optional webhook support
- subscription management
- signature verification
- webhook-driven invalidation and reconciliation

## Acceptance criteria for bootstrap pull request

A bootstrap implementation is acceptable when all of the following are true:

- repo has clear guidance docs
- code compiles
- tests pass
- CI exists
- `doctor` works
- `demo` or demo-backed `tui` works
- repo structure matches the architecture direction
- future Oura integration can land without a major rewrite

## Security and privacy requirements

- default to local storage only
- do not log access tokens or refresh tokens
- logging, `doctor`, and explicit error surfaces must redact raw personally identifying data
- secrets must have a dedicated seam for secure storage
- no telemetry by default
- avoid unnecessary network calls
- clearly separate demo data from real user data

## Quality requirements

- formatted code
- clippy clean under `-D warnings`
- test coverage for core routing and parsing paths
- docs updated with command and architecture changes
- no dead placeholder files that mislead future work

## Open questions intentionally left for implementation

- final dependency set and exact versions
- exact migration schema names
- exact TUI keybindings
- whether loopback auth lands in bootstrap or phase 2
- whether config uses TOML, env-first, or layered loading in the first pass

The implementation should pick the simplest robust answers, document them, and move forward.
