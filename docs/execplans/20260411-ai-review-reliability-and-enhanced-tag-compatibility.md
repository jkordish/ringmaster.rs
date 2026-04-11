# AI Review Reliability and Enhanced Tag Compatibility

## Goal

Improve `ai review` output quality for sparse live snapshots while restoring `enhanced_tag` sync compatibility with the current Oura V2 payload shape.

## Why

Live review artifacts are currently surfacing weak follow-up targets, occasionally citing irrelevant evidence, and losing local context because the enhanced-tag sync path is decoding against an outdated schema.

## Current state

- `EnhancedTagDocument` requires `day`, but Oura V2 now documents `start_day` as the required field.
- Snapshot follow-up investigate targets are derived from the first three review signals in alphabetical order.
- Review prompt guidance is minimal and does not strongly steer evidence selection or de-duplication.
- The eval harness only covers a tidy stale review fixture and does not stress sparse live-like snapshots.

## Desired state

- Enhanced-tag sync accepts current Oura payloads and continues to accept legacy-compatible payloads where practical.
- Snapshot follow-up targets stay deterministic, bounded, and better ranked.
- Review artifacts keep locally generated follow-up targets, avoid duplicated themes, and prefer stronger snapshot evidence.
- Eval fixtures catch these regressions before they ship again.

## Constraints

- Keep the project local-first and snapshot-bounded.
- Do not widen follow-up actions beyond existing bounded `review investigate` focuses.
- Keep the repo compileable between milestones.
- Update docs when prompt versions or AI/eval behavior changes.

## Risks

- Over-correcting follow-up ranking could hide useful drill-downs for sparse snapshots.
- Sanitizing provider output too aggressively could remove legitimate findings.
- Enhanced-tag compatibility changes must not break fixture-backed tests or older cached payloads.

## File plan

- `src/oura/models.rs`
- `src/snapshot.rs`
- `src/ai.rs`
- `src/ai_prompts.rs`
- `src/ai_prompts/review_prompt_v2.md`
- `src/ai_prompts/review_task_frame_v2.md`
- `src/eval.rs`
- `tests/fixtures/ai/*`
- `docs/OPENAI_INTEGRATION.md`
- `docs/EVALS.md`

## Milestones

- [x] Add enhanced-tag payload compatibility for `start_day` and cover it with tests.
- [x] Replace alphabetical follow-up target selection with deterministic ranked bounded targets.
- [x] Introduce review prompt v2 plus post-provider review artifact sanitization.
- [x] Extend eval fixtures and graders for sparse review behavior and deterministic follow-ups.
- [x] Update AI/eval docs and run verification.

## Verification

- `cargo test --all`
- `cargo run -- ai eval --fixture-dir tests/fixtures/ai`
- `cargo run -- doctor`

## Follow-up work

- Consider persisting raw payloads even when decode fails so future Oura schema drift is easier to inspect locally.
- Re-run a live `oura.enhanced_tags` sync against the persisted user database so `doctor` reflects the decoder fix instead of the prior stored failure state.
