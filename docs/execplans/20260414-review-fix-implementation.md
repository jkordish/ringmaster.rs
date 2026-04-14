# 20260414 review fix implementation

## Goal

Implement the high-confidence review fixes for auth refresh resilience, auth diagnostics visibility, webhook receiver request isolation, and publishability hygiene.

## Why

The current branch passes the mechanical suite but still has a few runtime and release-hygiene gaps that should be closed before updating the PR.

## Current state

- Auth status records `last_error`, but the main `doctor` and Ops surfaces do not show it.
- Authorized sessions only refresh when the stored access token is empty or explicitly stale.
- The webhook receiver serializes readiness and delivery handling behind one shared async mutex.
- The package metadata still presents the crate as publishable even though the project is app-first.

## Desired state

- Auth failures are visible in `doctor` and the Ops surface.
- Sessions with refresh tokens and missing expiry metadata proactively refresh.
- Webhook requests open short-lived stores instead of sharing one locked store handle.
- The package and README clearly state that the crate is app-first and not published as a general-purpose library.

## Constraints

- Keep the project local-first and privacy-first.
- Preserve compileable, non-breaking app behavior on the current branch.
- Add targeted regression tests for each behavior change.

## Risks

- Reopening the store per receiver request must not break existing webhook behavior.
- Packaging metadata changes should avoid disrupting local tooling or tests.

## File plan

- `src/lib.rs`
- `src/app.rs`
- `src/oura/auth.rs`
- `src/webhook/receiver.rs`
- `Cargo.toml`
- `README.md`

## Milestones

- [x] Add auth diagnostics and refresh fixes with regression coverage
- [x] Remove shared receiver store locking and update receiver tests
- [x] Mark the crate as app-first in packaging/docs and run full verification

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- Consider a future 401 refresh-and-retry path in the Oura client if upstream token behavior changes again.
- Revisit the public `lib` facade later if the project ever chooses to support downstream library consumers.
