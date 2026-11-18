# T-0075: Harden `forward::tests::later_bind_collision_releases_every_listener_without_spawning_tasks` so it deterministically proves every earlier listener is released after a later bind collision, without relying on timing-sensitive port reuse.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0075

## Outcome

Harden `forward::tests::later_bind_collision_releases_every_listener_without_spawning_tasks` so it deterministically proves every earlier listener is released after a later bind collision, without relying on timing-sensitive port reuse.

## Authority / Revision

User request 2026-08-25 / r1

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

Extracted deterministic listener acquisition and changed the regression to observe live kernel-assigned addresses, force a retained later collision, and prove every earlier listener is released by rebinding it. A deliberate listener-leak counterfactual failed immediately with `EADDRINUSE`; the corrected test passed 200 combined stress runs, all 12 forwarding tests, format, check, all-target Clippy, the full suite (519 unit and 67 runnable integration tests), build, diff check, and independent review.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0075 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
