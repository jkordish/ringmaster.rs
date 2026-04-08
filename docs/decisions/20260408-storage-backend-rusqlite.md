# Storage Backend Decision

Date: 2026-04-08

Status: accepted

## Decision

`ringmaster.rs` uses `rusqlite` as its storage layer for the bootstrap and early poll-first milestones.

We are intentionally not adopting Diesel at this stage, even though Diesel supports both SQLite and PostgreSQL.

## Context

The project is currently:

- local-first
- single-user
- SQLite-backed by design
- focused on a production-sane bootstrap rather than multi-backend abstraction

The current storage needs are also narrow:

- schema creation and migration running
- typed reads and writes for a small command surface
- deterministic local behavior for `doctor`, `demo`, `tui`, and `sync once`

There is interest in keeping the door open for PostgreSQL later. Diesel is a reasonable candidate if that becomes a real requirement, because it offers a typed query model across both SQLite and PostgreSQL.

That said, “future PostgreSQL support” is not the same as “backend differences disappear.” Migrations, SQL features, upsert behavior, indexing strategy, JSON handling, and performance tuning still diverge between SQLite and PostgreSQL. Choosing Diesel today would improve some query portability, but it would not remove backend-specific design pressure.

## Why `rusqlite` now

- It matches the current product stance directly: local-first and SQLite-first.
- It keeps the dependency graph smaller and easier to reason about at bootstrap time.
- It works well with explicit SQL and straightforward migration control.
- The current data access patterns are not yet complex enough to justify Diesel's schema and macro overhead.
- The existing store module boundaries already give us a clean seam for revisiting the storage implementation later.

## Consequences

Positive:

- faster bootstrap velocity
- simpler mental model for contributors
- fewer moving parts in the core local-first path
- direct control over schema and SQL during early iteration

Negative:

- no ORM-style compile-time query DSL
- no immediate shared abstraction for a hypothetical PostgreSQL backend
- a future backend expansion may require a deliberate storage refactor

## Guardrails

To keep a future storage change realistic, we should:

- keep SQL confined to `src/store/*`
- keep domain and view models storage-agnostic
- avoid leaking SQLite connection details into app or UI code
- prefer explicit query and migration boundaries over ad hoc storage calls
- avoid SQLite-only features unless they clearly earn their cost

## Revisit triggers

We should re-open this decision if one or more of the following become true:

- PostgreSQL becomes an actual product requirement rather than a hedge
- the query surface grows large enough that handwritten SQL meaningfully harms maintainability
- we need stronger compile-time guarantees around a more complex relational model
- sync/import throughput or concurrency needs force a broader storage redesign

## Alternatives considered

### Diesel

Pros:

- strong type safety for many query paths
- one ecosystem that supports both SQLite and PostgreSQL
- mature migration and schema tooling

Cons for this phase:

- higher setup and maintenance cost than the current needs justify
- backend portability is still partial, not automatic
- more abstraction than the current repository earns

### `sqlx`

We did not choose `sqlx` for bootstrap because the app does not currently need async database orchestration, pooling, or a cross-process service posture. For this local-first phase, `rusqlite` is the simpler fit.
