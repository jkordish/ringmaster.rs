# GOVERNANCE.md

## Purpose

This file defines how `ringmaster.rs` evolves so the project stays coherent instead of turning into an archaeological dig through abandoned abstractions.

## Project values

1. **Integrity over theater**
   - working code and accurate docs matter more than cargo-cult “architecture”
2. **Local-first**
   - prefer user control, local storage, and explicit behavior
3. **Small sharp changes**
   - smaller, verifiable increments beat sprawling speculative rewrites
4. **Typed boundaries**
   - separate UI, sync, and storage clearly
5. **Documentation as part of the change**
   - if behavior changes, docs change too

## Decision model

The maintainer is the final decision-maker.

For major work, the decision flow is:

1. write or update an ExecPlan
2. implement the smallest useful slice
3. verify with code, tests, and docs
4. merge only when the change meets the quality bar

## What counts as a major change

A change is “major” if it includes any of the following:

- schema or migration changes
- auth flow changes
- sync model changes
- new persistent configuration
- new top-level CLI command
- new TUI screen or major screen flow change
- dependency additions that materially affect architecture
- workspace or crate-layout changes

Major changes require:
- an ExecPlan
- spec updates when behavior changes
- explicit verification results

## Quality gates

### Required before merge

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`

### Required when relevant

- `cargo run -- doctor`
- demo mode smoke test
- migration test or manual migration verification
- manual TUI sanity pass for screen-flow changes

## Definition of done

A change is done only when:

- code compiles
- tests pass
- docs reflect reality
- no secret leakage is introduced
- no untracked TODOs are left in non-test code
- the change leaves the repo in a better state than it found it

## Dependency policy

Add dependencies reluctantly.

Every new dependency should satisfy all of the following:

- solves a real current problem
- is narrower than a homegrown solution would be
- fits the project’s local-first and production-sane stance
- does not force architectural complexity out of proportion to its value

Preferred posture:
- start lean
- add crates when implementation pressure justifies them
- remove crates that become unnecessary

## Security policy

- never log tokens, secrets, or raw personally identifying data unless explicitly redacted
- prefer local storage and OS secret storage seams
- keep webhook signing and verification isolated if/when added
- treat health data as sensitive even for a personal project

## Compatibility and releases

Until the first useful release, breaking internal refactors are allowed.

After the first useful release:
- aim for semantic versioning
- document breaking changes in the changelog
- provide migration notes for config or schema changes

## Documentation policy

These files must stay accurate:

- `README.md`
- `AGENTS.md`
- `SPEC.md`
- `GOVERNANCE.md`

If a change invalidates any of them, update them in the same change.

## Roadmap discipline

Do not confuse the roadmap with a promise.

It is acceptable to:
- cut scope
- defer features
- delete a half-baked direction

It is not acceptable to:
- leave the project in a broken state
- silently drift away from the documented architecture without updating the docs

## Escalation rule

If implementation pressure reveals the spec is wrong, stale, or too ambitious:

1. stop pretending otherwise
2. update the spec and plan
3. continue from the corrected design
