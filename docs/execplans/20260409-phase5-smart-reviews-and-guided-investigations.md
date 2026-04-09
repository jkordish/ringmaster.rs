# Phase 5 Smart Reviews And Guided Investigations

## Goal

Add a deterministic, evidence-backed smart review layer centered on a new `Review` product surface and `review` CLI family.

## Why

The repository already has local auth, sync, persistence, explainability, patterns, and ops. This pass turns those foundations into a product that can answer what matters today, what changed this week, and what evidence supports those answers without adding freeform chat or certainty theater.

## Current state

- Live sync currently covers `personal`, `daily`, `heartrate`, `workout`, `enhanced_tag`, and `session`.
- Derived state currently persists context events and pattern summaries only.
- The TUI currently includes Dashboard, Timeline, Trends, Explain, Patterns, and Ops.
- The current smart layer is limited to baseline comparisons and deterministic pattern summaries.
- Auth and capability truth are scope-level rather than endpoint-level.

## Desired state

- The app syncs, persists, and demos the additional high-signal Oura families needed for reviews:
  - `daily_stress`
  - `daily_resilience`
  - `sleep_time`
  - `daily_cardiovascular_age`
  - `vo2_max`
  - `rest_mode_period`
- A canonical metric and signal registry drives review behavior across CLI, TUI, and derivation.
- `derive rebuild` rebuilds persisted review signal snapshots in addition to context events and pattern summaries.
- A deterministic review engine produces structured outputs for:
  - `review today`
  - `review week`
  - `review investigate`
- The TUI gains a dedicated `Review` screen with Today, Week, and Investigate modes.
- Existing surfaces reuse the top smart outputs without duplicating the Review screen.

## Constraints

- No freeform chat assistant, text box, open-ended prompts, or hosted AI services.
- Keep the app local-first and single-crate.
- Preserve pure UI boundaries and the existing event/action/state/render flow.
- Keep auth scope handling explicit and honest; do not assume all scopes were granted.
- Do not hide stale, thin, or missing-data conditions.
- Keep the repo compileable after each milestone.
- No `unwrap`, `expect`, `todo!`, `panic!`, or `dbg!` in non-test code.

## Risks

- Oura exposes broad OAuth scopes while the new review families behave like narrower product capabilities. The registry must separate capability truth from data availability.
- Review ranking can become noisy if thresholds and tie-breakers are not explicit and stable.
- `sleep_time` and `rest_mode_period` are contextual signals rather than simple scalar metrics.
- Derived review state can drift from live tables unless `derive rebuild` and post-sync bounded rebuilds stay aligned.

## File plan

- `docs/execplans/20260409-phase5-smart-reviews-and-guided-investigations.md`
- `src/cli.rs`
- `src/lib.rs`
- `src/action.rs`
- `src/app.rs`
- `src/tui.rs`
- `src/components/mod.rs`
- `src/components/dashboard.rs`
- `src/components/explain.rs`
- `src/components/trends.rs`
- `src/components/review.rs`
- `src/review/*`
- `src/oura/models.rs`
- `src/oura/client.rs`
- `src/oura/sync.rs`
- `src/derive.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `tests/smoke_cli.rs`
- `tests/fixtures/phase5/*`

## Milestones

- [x] Milestone 1: add the phase-5 exec plan, schema changes, typed model and client support, fixture files, and sync coverage for the new review families.
- [x] Milestone 2: add the review registry, derived review feature snapshots, and `derive rebuild` integration.
- [x] Milestone 3: implement deterministic today and week review ranking, bounded investigations, and shared template rendering.
- [x] Milestone 4: add the `review` CLI family, the Review TUI screen, and concise smart-summary integration into existing surfaces.
- [x] Milestone 5: finish docs, run the full verification sweep, and repair issues.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- review today --demo`
- `cargo run -- review week --demo`
- `cargo run -- review investigate --focus readiness --demo`
- `cargo run -- derive rebuild --demo`

## Follow-up work

- Freeform chat and recommendation systems remain deferred.
- Notifications, packaging, installers, and release automation remain deferred.
- Broader Oura family expansion beyond the six review families remains deferred until a clear review use case exists.
