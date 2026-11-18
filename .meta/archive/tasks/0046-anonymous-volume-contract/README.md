# T-0046: Correct anonymous Feature volume serialization and test the Docker mount contract for anonymous and named volumes.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0046

## Outcome

Correct anonymous Feature volume serialization and test the Docker mount contract for anonymous and named volumes.

## Authority / Revision

Parent T-0043; user follow-up 2026-08-24 / r1

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

`FeatureMount::to_mount_string` now emits anonymous volumes as `type=volume,target=<path>` with no empty source field, while named volumes and bind mounts retain their source. Direct serialization and parsed runtime-label tests cover all three forms; the corrected anonymous assertion fails against the prior always-present formatting. Focused tests, format, clippy, full test (494 unit plus integration suites), build, diff review, and final `--mount` propagation inspection passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [anonymous volume contract brief](anonymous-volume-contract-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0046-anonymous-volume-contract/anonymous-volume-contract-brief.md`.

## Provenance

Migrated from the T-0046 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
