# T-0082: Add `dcc profile bootstrap` to exclusively create a minimal valid selected-profile configuration from an image, Dockerfile, or existing valid parent profile.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0082

## Outcome

Add `dcc profile bootstrap` to exclusively create a minimal valid selected-profile configuration from an image, Dockerfile, or existing valid parent profile.

## Authority / Revision

User request 2026-09-08 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added an exactly-one source contract for `--image`, `--dockerfile`, or `--extends`; direct `-p/--profile` targeting; valid-parent inheritance; exclusive no-overwrite creation; dry-run/debug support; user documentation; and nine CLI regressions. Counterfactual tests failed because bootstrap was absent, then the focused suite, format, check, all-target Clippy, full suite (555 unit plus 77 runnable integration tests), build, help smoke, diff check, and focused review passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0082 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
