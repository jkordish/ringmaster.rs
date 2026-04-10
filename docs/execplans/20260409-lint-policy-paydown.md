# Lint Policy Paydown

## Goal

Reduce the crate-root Clippy allow list so it reflects real project policy instead of accumulated noise suppression.

## Why

The current root-level `#![allow(...)]` block is broad enough to hide which lints are truly intentional versus which ones are stale. That makes lint policy harder to trust and harder to maintain.

## Current state

`src/lib.rs` enables several aggressive Clippy groups and then disables a large set of lints globally. Most non-root allow sites are test-only panic/unwrap allowances.

## Desired state

Remove stale global allows, keep only the project-wide policy choices that still earn their keep, and localize any remaining necessary suppressions to the narrowest files or items.

## Constraints

- Keep the repo compiling cleanly under `cargo clippy --all-targets --all-features -- -D warnings`.
- Do not churn test-only panic allowances unless they block the root cleanup.
- Avoid speculative policy changes that would require broad doc rewrites.

## Risks

- Removing a root allow can surface many warnings across multiple modules.
- Some lints may be intentionally suppressed for pragmatic reasons and need local treatment instead of outright deletion.

## File plan

- `docs/execplans/20260409-lint-policy-paydown.md`
- `src/lib.rs`
- additional source files only if a localized allow or small fix is needed

## Milestones

- [x] create the plan and identify candidate stale root allows
- [x] remove stale global allows and localize any necessary remaining suppressions
- [x] run verification and update the plan with what stayed deferred

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`

## Follow-up work

- Revisit whether `clippy::pedantic`, `clippy::nursery`, and `clippy::cargo` should stay enabled crate-wide or move to a more curated lint set.
- Consider a second pass on migration/test naming if we want to reduce historical `phase*` language in non-runtime contexts.
- The crate still keeps global allows for `map_unwrap_or`, `missing_errors_doc`, `must_use_candidate`, and `uninlined_format_args` because they currently act as broad style/documentation policy choices rather than isolated code smells.
- This pass retired four crate-wide allows: `derive_partial_eq_without_eq`, `ignored_unit_patterns`, `match_same_arms`, and `option_if_let_else`, replacing them with targeted code fixes and one local allow on `AppModel`.
