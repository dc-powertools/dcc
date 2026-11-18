# T-0080: Replace T-0075's still-racy released-address rebind assertion with a deterministic listener-cleanup oracle that cannot be invalidated by concurrent port acquisition.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0080

## Outcome

Replace T-0075's still-racy released-address rebind assertion with a deterministic listener-cleanup oracle that cannot be invalidated by concurrent port acquisition.

## Authority / Revision

Required verification correction discovered during T-0073; parent T-0073 and user drain request 2026-08-25 / r1

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

Replaced released-address rebinding with an exact-object `Arc`/`Weak` listener-lifetime oracle while preserving bind-all-before-spawn behavior. The retained-owner counterfactual failed as intended; 200 concurrent stress runs, a clean first-run full suite under port churn (555 unit and 68 runnable integration tests), format, check, all-target Clippy, build, diff, and independent review passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0080 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
