# T-0009: Implement durable runtime lifecycle commands: `start`, `stop`, `run`, `exec`, `attach`, one-shot bookkeeping, and `--keep` promotion.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0009

## Outcome

Implement durable runtime lifecycle commands: `start`, `stop`, `run`, `exec`, `attach`, one-shot bookkeeping, and `--keep` promotion.

## Authority / Revision

Parent T-0004 / r1

## Status

Done

## Depends On

T-0008

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added `start`, `attach`, `--keep`/`-k`, durable promotion, existing-container reuse, one-shot active-command bookkeeping, scoped startup/attach hooks, docs, and tests. Verified fmt, clippy, tests, and build.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [runtime lifecycle brief](runtime-lifecycle-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0009-runtime-lifecycle/runtime-lifecycle-brief.md`.

## Provenance

Migrated from the T-0009 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
