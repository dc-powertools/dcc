# T-0119: Diagnose one-shot drain CI failure with status 137 before command marker.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0119

## Outcome

Diagnose one-shot drain CI failure with status 137 before command marker.

## Authority / Revision

User diagnosis request 2026-11-17 / r1

## Status

Done

## Depends On

None

## Route / Risk

Discover / Low

## Approval Or Blocker

No product implementation requested

## Next Safe Action

Stop; registration/drain repair remains in open T-0113.

## Result

Actual dcc 0.3.1 + exact test command returned 137 before marker when checkpoints forced stale count plus new ARRIVED. Active record existed, readiness/snapshot succeeded, container exited 0. CI-specific attribution limited without event logs.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality one shot registration ci](quality-one-shot-registration-ci.md) — Verification and review evidence.

Additional original references: `.meta/archive/tasks/0119-diagnose-one-shot-drain-ci-failure-with-status/quality-one-shot-registration-ci.md`.

## Provenance

Migrated from the T-0119 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
