# Telemetry Gap Closure

## Goal

Close the remaining telemetry and composition gaps on this branch by wiring real HRV, respiratory-rate, and SpO2 data into the local model, then moving Explain, Patterns, and Review onto the same telemetry-first screen language already used by Dashboard.

## Why

The current redesign intentionally stopped short of inventing physiology continuity that the app did not yet persist. With the Oura surface now verified, we can replace those honest placeholder shells with real local-first telemetry while also unifying the remaining prose-heavy screens with the newer monitoring composition system.

## Current state

- `daily_stress`, `daily_resilience`, `daily_cardiovascular_age`, and `vo2_max` already sync and persist, but `LiveSnapshot` does not load or surface most of them.
- Dashboard still renders HRV and respiratory rate as placeholder shells because no persisted local series exists yet.
- SpO2 is only represented in capability and coverage status, not as a persisted metric family or panel.
- Explain, Patterns, and Review share updated chrome, but they still use older string-heavy list/card layouts rather than telemetry-first panel composition.
- Oura V2 exposes:
  - `sleep.average_hrv`
  - `sleep.average_breath`
  - `daily_spo2.spo2_percentage.average`
  - `daily_spo2.breathing_disturbance_index`

## Desired state

- The local store persists HRV and respiratory-rate values from the `sleep` collection and SpO2 values from `daily_spo2`.
- `LiveSnapshot` and dashboard models consume those persisted records with honest freshness and missing-data semantics.
- Dashboard physiology panels use real local values and footer inspector details instead of static placeholder copy.
- Explain, Patterns, and Review render as telemetry-first panels with stable degraded states and shared visual language.
- Docs and tests reflect the new behavior and no longer describe these gaps as current.

## Constraints

- Keep the app local-first and privacy-first.
- Preserve pure UI rendering boundaries.
- Do not fabricate continuity or infer unsupported data.
- Ship compileable slices and keep tests updated as behavior changes.
- Preserve existing navigation regions unless a pane becomes materially more operable.

## Risks

- Adding new persisted physiology tables and snapshot fields touches migrations, sync, store, and app model code in one pass.
- The existing dashboard and footer-inspector assumptions may hide placeholder-specific logic that needs to be updated consistently.
- Explain, Patterns, and Review have broad screen models in `src/app.rs`, so screen composition changes can sprawl if not kept disciplined.

## File plan

- `src/oura/models.rs`
- `src/oura/client.rs`
- `src/oura/sync.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `src/app.rs`
- `src/components/dashboard.rs`
- `src/components/explain.rs`
- `src/components/patterns.rs`
- `src/components/review.rs`
- `src/ui/telemetry.rs`
- `README.md`
- `docs/STATUS.md`
- `docs/ARCHITECTURE.md`
- `docs/IMPLEMENT.md`
- relevant fixtures and tests

## Milestones

- [x] Add persisted HRV, respiratory-rate, and SpO2 plumbing across Oura models, sync, migrations, and store queries.
- [x] Load the new physiology records into `LiveSnapshot` and replace dashboard placeholder panels with real telemetry state.
- [x] Refactor Explain, Patterns, and Review onto shared telemetry-first composition without destabilizing navigation semantics.
- [x] Update docs, fixtures, and tests, then run the verification suite.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- targeted snapshot coverage for Dashboard, Explain, Patterns, Review, and Ops states

## Follow-up work

- Consider richer timeline or trends views for new physiology families once the baseline dashboard and review surfaces settle.
- Revisit moving large screen model structs out of `src/app.rs` after the telemetry-first composition stabilizes.

## Progress notes

- Verified Oura V2 support for nightly HRV and respiratory rate through `sleep`, and daily oxygen saturation through `daily_spo2`.
- Added persisted `sleep_periods` and `daily_spo2` tables plus typed sync/client/store plumbing.
- Promoted sleep physiology, resilience, cardiovascular age, and VO2 max into `LiveSnapshot` so dashboard and review-adjacent surfaces can read real local telemetry.
- Rebuilt Explain, Patterns, and Review onto the shared telemetry panel composition while keeping their navigation regions stable.
- Added CLI smoke coverage for Explain / Patterns / Review snapshot generation.
- Verified with `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`, and `cargo run -- doctor`.
