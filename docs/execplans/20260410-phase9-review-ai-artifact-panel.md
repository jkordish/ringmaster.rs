# Phase 9: Review AI Artifact Panel

## Goal

Add a narrow, read-only AI artifact panel to the Review screen so the selected day can show whether a saved AI artifact exists, what it concluded in compact form, and what provenance it carries.

## Why

Phase 8 made snapshots, AI runs, reports, and evals durable, but the TUI still cannot surface that saved artifact context inside the existing day-oriented Review flow.

## Current state

- `ai_artifacts` and `snapshot_exports` persist AI run metadata, compact summary cache, and snapshot linkage.
- `ai runs list/show` expose that registry through the CLI.
- The Review screen remains local and read-only, but it only shows deterministic review and investigation output.
- No TUI overlay or artifact browser exists today.

## Desired state

- Review shows a compact AI artifact panel for the selected day.
- The panel is provenance-first and read-only.
- Day switching reuses preloaded local state instead of performing fresh store reads from the widget path.
- No schema changes, provider calls, or new top-level screen flow land in this pass.

## Constraints

- Keep the project local-first and privacy-first.
- Keep rendering pure: no HTTP, DB writes, or token refresh inside components.
- Stay inside the existing Review screen for the first slice.
- Reuse `summary_cache` and `overview`; do not invent a new artifact rendering pipeline.
- Defer overlay/detail browser and eval-backed trust rows.

## Risks

- Day-to-snapshot matching could become confusing if compare artifacts are not explicit about which side matched the selected day.
- Pulling artifact data lazily from the Review widget would violate the current app flow and make day navigation inconsistent.
- Long run ids or snapshot hashes could make compact layouts noisy if the panel is not constrained carefully.

## File plan

- `docs/execplans/20260410-phase9-review-ai-artifact-panel.md`
- `src/store/queries.rs`
- `src/app.rs`
- `src/components/review.rs`
- `src/tui.rs`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `README.md`

## Milestones

- [x] Milestone 1: add the ExecPlan, typed day-to-artifact store query, and store-level tests.
- [x] Milestone 2: preload day-keyed artifact summaries into the live snapshot and map them into a Review presentation view.
- [x] Milestone 3: render the Review-only AI artifact panel for compact and wide layouts and add snapshot coverage.
- [x] Milestone 4: update docs and verification notes.

## Verification

- `cargo fmt --all`
- Targeted verification completed during implementation:
  - `cargo test --all store::queries::tests::latest_ai_artifact_for_anchor_day_prefers_newest_review_for_matching_day -- --exact`
  - `cargo test --all app::tests::day_actions_update_shared_selected_day -- --exact`
  - `cargo test --all tui::tests::renders_review_screen_with_ranked_cards -- --exact`
  - `cargo test --all tui::tests::compact_review_snapshot_keeps_tabs_and_multiple_cards_visible -- --exact`
- Full verification completed at closeout:
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all`
  - `cargo run -- doctor`

## Follow-up work

- Add an optional read-only artifact detail overlay once the small panel settles.
- Consider a dedicated TUI artifact browser only after the Review-panel semantics are proven useful.
- Revisit eval-backed trust context once version-matching rules are explicit enough to avoid confidence theater.
