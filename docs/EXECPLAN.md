# EXECPLAN.md

Use an ExecPlan for any change that is more than a quick isolated fix.

## When an ExecPlan is required

Write one before implementation if the task includes any of the following:

- touches more than one module area
- adds or changes schema or migrations
- changes auth or sync flow
- adds a major dependency
- changes CLI surface
- adds a screen or significant UI flow
- is likely to take more than about 30 minutes

## Where to put it

Create plans under:

`docs/execplans/YYYYMMDD-<slug>.md`

Example:

`docs/execplans/20260408-bootstrap-shell.md`

## Template

```md
# <title>

## Goal

What is being built or changed?

## Why

Why is the change needed now?

## Current state

What exists today?

## Desired state

What should exist after the change?

## Constraints

Technical, product, security, or scope constraints.

## Risks

What could go wrong?

## File plan

List the files and modules expected to change.

## Milestones

- [ ] milestone 1
- [ ] milestone 2
- [ ] milestone 3

## Verification

Commands to run and manual checks to perform.

## Follow-up work

Explicitly list deferred work instead of hiding it in code comments.
```

## Rules

- Keep the plan short and concrete.
- Update the plan when scope changes.
- Close the loop: when implementation ends, mark what completed and what was deferred.
- If the work invalidates the original plan, revise the plan instead of pretending the drift did not happen.
