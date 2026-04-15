# Doctor Auth Redaction

## Goal

Redact persisted auth identity fields from `cargo run -- doctor` while keeping auth-health diagnostics useful.

## Why

The current doctor report prints raw `account_id` and `account_email` values from the persisted auth session, which violates the project privacy policy and creates copy-paste leakage risk.

## Current state

`run_doctor` loads `AuthStatus` from persisted auth metadata and `doctor_auth_section` renders account identifiers verbatim. Existing tests cover auth error visibility but not redaction.

## Desired state

Doctor output keeps the auth-health summary, but raw identity fields render as redacted markers instead of plaintext values. Regression coverage prevents future reintroduction.

## Constraints

- Keep the project local-first and privacy-first.
- Do not weaken doctor usefulness for auth troubleshooting.
- Ship compileable changes with updated docs and tests.

## Risks

- Over-redacting could hide whether metadata is present at all.
- Test coverage could miss the live report path if only helper functions change.

## File plan

- `src/lib.rs`
- `README.md`
- `SPEC.md`
- `docs/execplans/20260415-doctor-auth-redaction.md`

## Milestones

- [x] Add the exec plan and document the privacy intent.
- [x] Redact doctor auth identity fields in the report output.
- [x] Add regression coverage and rerun fmt, clippy, tests, and doctor.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- Consider whether other operational surfaces should explicitly expose a `present`/`missing` marker for auth identity fields without revealing values.
