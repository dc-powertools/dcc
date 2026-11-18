# T-0098: Update the TLS smoke-test Docker stub for the running-runtime build preflight.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0098

## Outcome

Update the TLS smoke-test Docker stub for the running-runtime build preflight.

## Authority / Revision

User request 2026-09-28 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / Low

## Approval Or Blocker

None

## Next Safe Action

Stop; stub update complete.

## Result

Stub accepts only the exact fixture-scoped runtime query, returning no containers; other Docker calls still mark failure and exit 97. The missing/wrong CA test reproduced the CI failure before the change and passed afterward. Format, locked check, all-target Clippy, all 702 default runnable tests, locked build, and focused diff review passed. No containers launched.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0098 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
