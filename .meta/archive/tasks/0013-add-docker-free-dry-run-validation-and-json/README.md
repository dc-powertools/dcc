# T-0013: Add Docker-free dry-run validation and JSON output for CLI acceptance tests.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0013

## Outcome

Add Docker-free dry-run validation and JSON output for CLI acceptance tests.

## Authority / Revision

User request 2026-07-20 / r1

## Status

Done

## Depends On

T-0012

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added global `--dry-run` and `--format text\|json` flags, stable dry-run reports, Docker-boundary exits for build/runtime/stop commands, JSON output for `dcc id`, docs, and CLI tests that use dry-run success instead of fragile Docker error parsing. Kept ignored Docker smokes for real build/exec/run coverage. Verified fmt, tests, clippy, build, help output, dry-run JSON output, ignored-test listing, and workflow lint with `actionlint 1.7.12`.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0013 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
