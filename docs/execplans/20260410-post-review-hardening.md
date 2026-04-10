# Post-Review Hardening for Phase 8/9

## Goal

Close the review findings around snapshot lineage preservation, AI run identity/history retention, compare-report freshness/trust summaries, Review follow-up routing, demo AI run stability, snapshot catalog chronology, snapshot export coverage, compare-report evidence rendering, AI run prefix resolution in report export, and transient provider retry handling.

## Why

The Phase 8/9 slices shipped useful new surfaces, but review caught a data-loss bug in snapshot recataloging, a history-collapse bug in AI artifact persistence, and additional correctness gaps in report/export, follow-up command routing, demo seeding stability, snapshot catalog ordering, wide-range snapshot export derivation, compare evidence rendering, AI-run source resolution, and retry behavior under temporary provider failures.

## Current state

- Re-cataloging a snapshot with no provenance refs overwrites `snapshot_provenance_refs` and provenance summary metadata.
- AI artifact IDs can collide across repeated identical runs, collapsing run history.
- Compare reports reuse snapshot A freshness/trust metadata even when snapshot B is the weaker input.
- Stress and recovery review signals route to `readiness` instead of `stress` / `recovery`.
- Demo-seeded AI runs use per-execution IDs, so `ai runs list --demo` output cannot be reused across invocations.
- Snapshot catalog upserts overwrite `created_at`, which can backdate existing catalog rows during metadata refresh paths.
- `snapshot export` rebuilds derived review artifacts around the anchor day and can drop earlier-range context/pattern/review records for wide exports.
- Compare report export only renders top-level supporting evidence, dropping evidence attached to individual material differences.
- `report export --from-ai-run` requires an exact AI run id even though the CLI shows shortened prefixes elsewhere.
- AI retries ignore transient 408/5xx provider responses, so temporary OpenAI blips abort immediately.

## Desired state

- Metadata-only snapshot upserts preserve existing provenance refs and lineage summary context.
- AI artifact persistence records each execution as its own history entry.
- Compare reports surface freshness/trust context from both input snapshots.
- Follow-up commands open the correct investigation focus for stress and recovery signals.
- Demo-seeded AI runs keep stable IDs across repeated invocations of the seeded temporary library.
- Snapshot catalog upserts preserve the original `created_at` chronology for existing rows.
- Snapshot exports include derived context events, pattern summaries, and review signals across the full requested day range.
- Compare reports render evidence whether it is attached at the top level or per material difference.
- `report export --from-ai-run` accepts the same unique prefixes that `ai runs show` accepts.
- AI retries treat transient OpenAI 408/500/502/503/504 failures as retryable when `ai.max_retries` is configured.

## Constraints

- Keep storage and UI boundaries intact.
- No schema changes or new top-level commands.
- Preserve local-first behavior and existing report/export surfaces.
- Keep the retry logic narrow so permanent client/config errors still fail fast.

## Risks

- Snapshot upsert semantics could become ambiguous if callers need to intentionally clear provenance later.
- Artifact ID changes must still leave `ai runs show` and report export stable for persisted rows.
- Compare-report summary text should stay compact while remaining provenance-first.
- Demo-only stability needs to avoid regressing real run-history preservation.
- Preserving catalog `created_at` should not block metadata refreshes for the rest of the row.
- Re-deriving export artifacts across the requested range must not accidentally widen the snapshot beyond the requested scope.
- Prefix resolution in report export must still reject ambiguous shortened ids.
- Retry behavior must not hide persistent provider-side schema or auth failures.

## File plan

- `docs/execplans/20260410-post-review-hardening.md`
- `src/store/queries.rs`
- `src/ai.rs`
- `src/lib.rs`
- `tests/smoke_cli.rs`
- `src/report.rs`
- `src/snapshot.rs`
- `src/derive.rs`

## Milestones

- [x] Preserve provenance on metadata-only snapshot recataloging and cover it with store tests.
- [x] Give AI artifact IDs execution-specific identity and cover repeated-run history with tests.
- [x] Make compare reports summarize both input snapshots and route stress/recovery follow-ups correctly.
- [x] Keep demo-seeded AI run IDs stable across invocations and preserve snapshot catalog chronology on upsert.
- [x] Export full-range derived snapshot artifacts, render compare evidence comprehensively, accept AI-run prefixes in report export, and retry transient 408/5xx provider failures.
- [x] Run focused verification plus full repo checks.

## Verification

- `cargo fmt --all`
- `cargo test --all store::queries::tests::analysis_store_preserves_provenance_on_metadata_only_snapshot_upsert -- --exact`
- `cargo test --all ai::tests::artifact_ids_include_run_identity_even_for_identical_payloads -- --exact`
- `cargo test --all report::tests::compare_reports_surface_freshness_and_trust_for_both_snapshots -- --exact`
- `cargo test --all snapshot::tests::follow_up_targets_route_stress_and_recovery_signals_to_matching_focus -- --exact`
- `cargo test --all store::queries::tests::analysis_store_preserves_snapshot_created_at_on_upsert -- --exact`
- `cargo test --test smoke_cli ai_runs_show_demo_accepts_id_listed_by_previous_demo_invocation -- --exact`
- `cargo test --all snapshot::tests::snapshot_export_derives_artifacts_across_requested_range -- --exact`
- `cargo test --all report::tests::compare_reports_include_per_difference_evidence_when_top_level_evidence_is_empty -- --exact`
- `cargo test --all ai::tests::retryable_error_includes_transient_status_codes -- --exact`
- `cargo test --all tests::report_export_from_ai_run_accepts_unique_prefixes -- --exact`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

All targeted checks and the full verification suite completed successfully at closeout.

## Follow-up work

- If we ever need explicit provenance clearing, add a dedicated store API rather than overloading empty provenance slices.
- Consider a first-class persisted run identity field if future tooling needs something shorter or more human-oriented than the artifact hash.
- If demo seeding grows beyond two artifacts, consider a more explicit deterministic demo run identity scheme instead of reusing snapshot-generated timestamps.
- If provider retry policy expands beyond OpenAI, consider a typed transient-status helper instead of message parsing.
