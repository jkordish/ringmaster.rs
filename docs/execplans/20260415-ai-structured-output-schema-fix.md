# AI Structured Output Schema Fix

## Goal

Fix OpenAI Responses API structured-output failures caused by generated JSON Schema that does not meet the strict supported subset.

## Why

The AI compare flow is currently failing before inference with an invalid `response_format` schema error. The same schema-generation path is shared by review and follow-up, so the fix should be centralized and regression-tested.

## Current state

`src/ai.rs` sends raw `schemars` output as the structured-output schema. That output can leave object properties optional in ways that OpenAI strict structured outputs reject.

## Desired state

- AI review, compare, and follow-up requests all send schemas that satisfy OpenAI strict structured-output requirements.
- Optional Rust fields are represented as required-but-nullable in the emitted schema.
- Recursive tests catch unsupported object shapes before runtime.

## Constraints

- Keep the existing typed artifact structs as the source of truth.
- Avoid changing the outward artifact payload contract unless the API subset requires it.
- Preserve dry-run and fixture behavior.

## Risks

- Over-normalizing the schema could distort intended enum/union behavior.
- A partial fix for compare alone could leave review/follow-up broken later.

## File plan

- `src/ai.rs`
- `docs/OPENAI_INTEGRATION.md`
- `docs/execplans/20260415-ai-structured-output-schema-fix.md`

## Milestones

- [x] Add schema normalization for OpenAI strict structured outputs
- [x] Add recursive schema-validity regression tests
- [x] Re-run targeted and full verification

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- If OpenAI tightens the supported schema subset further, add an explicit local validator that mirrors the currently documented contract.
