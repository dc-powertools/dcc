# T-0014: Add expanded Docker behavior smoke tests for the completed rewrite.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0014

## Outcome

Add expanded Docker behavior smoke tests for the completed rewrite.

## Authority / Revision

User request 2026-07-20 / r1

## Status

Done

## Depends On

T-0012, T-0013

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added ignored `tests/docker_smoke.rs` coverage for project named commands, lifecycle hook phase separation, state directory/file persistence across one-shot containers and refresh-only builds, durable vs one-shot container reuse, `workspaceFolder` workdir behavior, runtime environment substitution, and local Feature command/state metadata. Updated CI to run the suite serially on GitHub Docker runners. Verified fmt, tests, clippy, build, actionlint, and ignored-test listings locally; live Docker execution remains delegated to GitHub runners.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0014 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
