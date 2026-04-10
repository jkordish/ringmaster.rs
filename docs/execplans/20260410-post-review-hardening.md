# Post-Review Hardening for Phase 8/9

## Goal

Close the review findings around snapshot lineage preservation, AI run identity/history retention, compare-report freshness/trust summaries, and Review follow-up routing.

## Why

The Phase 8/9 slices shipped useful new surfaces, but review caught a data-loss bug in snapshot recataloging, a history-collapse bug in AI artifact persistence, and two smaller correctness gaps in report/export and follow-up command routing.

## Current state

- Re-cataloging a snapshot with no provenance refs overwrites `snapshot_provenance_refs` and provenance summary metadata.
- AI artifact IDs can collide across repeated identical runs, collapsing run history.
- Compare reports reuse snapshot A freshness/trust metadata even when snapshot B is the weaker input.
- Stress and recovery review signals route to `readiness` instead of `stress` / `recovery`.

## Desired state

- Metadata-only snapshot upserts preserve existing provenance refs and lineage summary context.
- AI artifact persistence records each execution as its own history entry.
- Compare reports surface freshness/trust context from both input snapshots.
- Follow-up commands open the correct investigation focus for stress and recovery signals.

## Constraints

- Keep storage and UI boundaries intact.
- No schema changes or new top-level commands.
- Preserve local-first behavior and existing report/export surfaces.

## Risks

- Snapshot upsert semantics could become ambiguous if callers need to intentionally clear provenance later.
- Artifact ID changes must still leave `ai runs show` and report export stable for persisted rows.
- Compare-report summary text should stay compact while remaining provenance-first.

## File plan

- `docs/execplans/20260410-post-review-hardening.md`
- `src/store/queries.rs`
- `src/ai.rs`
- `src/report.rs`
- `src/snapshot.rs`

## Milestones

- [x] Preserve provenance on metadata-only snapshot recataloging and cover it with store tests.
- [x] Give AI artifact IDs execution-specific identity and cover repeated-run history with tests.
- [x] Make compare reports summarize both input snapshots and route stress/recovery follow-ups correctly.
- [x] Run focused verification plus full repo checks.

## Verification

- `cargo fmt --all`
- `cargo test --all store::queries::tests::analysis_store_preserves_provenance_on_metadata_only_snapshot_upsert -- --exact`
- `cargo test --all ai::tests::artifact_ids_include_run_identity_even_for_identical_payloads -- --exact`
- `cargo test --all report::tests::compare_reports_surface_freshness_and_trust_for_both_snapshots -- --exact`
- `cargo test --all snapshot::tests::follow_up_targets_route_stress_and_recovery_signals_to_matching_focus -- --exact`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

All targeted checks and the full verification suite completed successfully at closeout.

## Follow-up work

- If we ever need explicit provenance clearing, add a dedicated store API rather than overloading empty provenance slices.
- Consider a first-class persisted run identity field if future tooling needs something shorter or more human-oriented than the artifact hash.
