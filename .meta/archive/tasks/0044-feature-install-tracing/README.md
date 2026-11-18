# T-0044: Remove unconditional shell tracing from Feature installation and replace trace-shape tests with coverage that protects install behavior without exposing secrets.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0044

## Outcome

Remove unconditional shell tracing from Feature installation and replace trace-shape tests with coverage that protects install behavior without exposing secrets.

## Authority / Revision

Parent T-0043; user follow-up 2026-08-24 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Removed `inject_trace` and now archives Feature `install.sh` bytes unchanged. Replaced trace-shape tests with archive byte-preservation, POSIX/Bash/shebang-less execution, and no-secret-output coverage; a deliberate `set -x` negative control proves the secret assertion catches the prior behavior. Focused 35 tests, clippy, full test (480 unit plus integration suites), build, format, diff, and secret-output review passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [feature install tracing brief](feature-install-tracing-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0044-feature-install-tracing/feature-install-tracing-brief.md`.

## Provenance

Migrated from the T-0044 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
