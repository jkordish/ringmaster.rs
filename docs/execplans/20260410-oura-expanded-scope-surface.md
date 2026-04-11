# Oura Expanded Scope Surface

## Goal

Add Oura's newer scope names to the default requested scope set and expose them as first-class capability entries in auth, doctor, demo, and fixture-backed surfaces.

## Why

The current app only requests and displays the older baseline scopes even though the Oura developer surface now exposes additional access areas such as `email`, `spo2`, `ring_configuration`, `stress`, and `heart_health`.

## Current state

- `default_requested_scopes()` only includes the older baseline set.
- `CapabilityKind` only models the older capability entries.
- demo and fixture capability surfaces do not reflect the newer scope names.

## Desired state

- the default auth request includes the expanded Oura scope set
- auth/doctor/TUI capability reporting shows the newer scope areas with polished labels
- fixture and demo capability reports stay honest about currently supported vs future-ready scopes

## Constraints

- keep the app privacy-first and least-surprising
- do not invent live sync support for datasets we still do not fetch
- preserve existing sync behavior for already-supported families

## Risks

- scope/capability naming can drift from the current Oura developer surface
- demo and fixture tests may become brittle if scope ordering changes unexpectedly

## File plan

- `docs/execplans/20260410-oura-expanded-scope-surface.md`
- `src/config.rs`
- `src/oura/models.rs`
- `src/oura/client.rs`
- `src/lib.rs`
- `src/app.rs`
- `src/tui.rs`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`

## Milestones

- [x] expand the default requested scopes and capability model
- [x] align demo and fixture capability reporting with the new scope surface
- [x] update docs and rerun full verification

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo build`
- `cargo run -- doctor`

## Follow-up work

- add real sync/model/UI support for `spo2` and `ring_configuration`
- decide whether `email` should remain requested by default or move behind a stricter privacy toggle
