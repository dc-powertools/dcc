# T-0055: Restore implicit Docker runtime limits of 4 GiB memory and 2 CPUs, retain explicit overrides, and document the defaults as intended product behavior.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0055

## Outcome

Restore implicit Docker runtime limits of 4 GiB memory and 2 CPUs, retain explicit overrides, and document the defaults as intended product behavior.

## Authority / Revision

User request 2026-08-24 / r1

## Status

Done

## Depends On

T-0050

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

`run`, `exec`, `attach`, and `start` now create containers with default `--memory 4g --cpus 2`; either explicit option overrides only that resource. CLI help and docs expose the defaults, and fake-Docker tests cover all entry points, ordering, uniqueness, and partial overrides. Format, check, clippy, full tests, build, and diff checks passed; live Docker enforcement remains environment-bound residual evidence.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality default docker limits quality](quality-default-docker-limits-quality.md) — Verification and review evidence.
- [default docker limits brief](default-docker-limits-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0055-default-docker-limits/default-docker-limits-brief.md`; `.meta/archive/tasks/0055-default-docker-limits/quality-default-docker-limits-quality.md`.

## Provenance

Migrated from the T-0055 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
