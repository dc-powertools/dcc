# T-0056: Restore an error for missing `${containerEnv:VAR}` values without a default and document the intended substitution contract.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0056

## Outcome

Restore an error for missing `${containerEnv:VAR}` values without a default and document the intended substitution contract.

## Authority / Revision

User request 2026-08-24 / r1

## Status

Done

## Depends On

T-0051

## Route / Risk

Initiative / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Missing `${containerEnv:VAR}` now errors unless an explicit default is supplied; explicit empty defaults remain allowed, present-empty stays empty, and present-nonempty wins. Fallible resolution with consumer context covers state, workspaceFolder, runArgs, command arguments, mounts, remoteEnv, build/runtime lifecycle hooks, and Feature equivalents. Decision 0006 supersedes 0005. Format, check, clippy, full tests, build, and diff checks passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [decision 0006 require missing container env default](decision-0006-require-missing-container-env-default.md) — Historical decision rationale.
- [quality missing container env error quality](quality-missing-container-env-error-quality.md) — Verification and review evidence.
- [missing container env error brief](missing-container-env-error-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0056-missing-container-env-error/missing-container-env-error-brief.md`; `.meta/archive/tasks/0056-missing-container-env-error/decision-0006-require-missing-container-env-default.md`; `.meta/archive/tasks/0056-missing-container-env-error/quality-missing-container-env-error-quality.md`.

## Provenance

Migrated from the T-0056 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
