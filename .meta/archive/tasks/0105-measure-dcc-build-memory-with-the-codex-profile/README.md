# T-0105: Measure dcc build memory with the Codex profile.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0105

## Outcome

Measure dcc build memory with the Codex profile.

## Authority / Revision

User request 2026-11-16 / r1

## Status

Done

## Depends On

None

## Route / Risk

Discover / Medium

## Approval Or Blocker

User authorizes build measurement.

## Next Safe Action

Stop; measurement complete.

## Result

Debug dcc cached Codex build peaked at 988.4 MiB RSS with 965.7 MiB state, versus 15.4 MiB baseline. In-memory tar state hashing explains scaling; both builds exited 0. No product changes.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality build memory](quality-build-memory.md) — Verification and review evidence.

Additional original references: `.meta/archive/tasks/0105-measure-dcc-build-memory-with-the-codex-profile/quality-build-memory.md`.

## Provenance

Migrated from the T-0105 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
