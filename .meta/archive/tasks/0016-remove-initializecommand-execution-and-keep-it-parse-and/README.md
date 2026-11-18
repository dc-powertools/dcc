# T-0016: Remove `initializeCommand` execution and keep it parse-and-warn only.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0016

## Outcome

Remove `initializeCommand` execution and keep it parse-and-warn only.

## Authority / Revision

User correction 2026-07-20 / r1

## Status

Done

## Depends On

T-0004

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Removed host execution of `initializeCommand`, deleted the host lifecycle executor, excluded `initializeCommand` from runtime `${containerEnv:...}` probing and debug lifecycle output, preserved parser compatibility with an unsupported-field warning, and updated README/architecture docs. Added CLI coverage for the warning and Docker smoke coverage that would fail if the hook mutates the host workspace. Verified fmt, tests, clippy, diff check, and stale-reference search.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0016 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
