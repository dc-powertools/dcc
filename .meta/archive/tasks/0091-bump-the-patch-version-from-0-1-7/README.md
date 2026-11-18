# T-0091: Bump the patch version from 0.1.7 to 0.1.8 in a separate local commit after the bootstrap fix.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0091

## Outcome

Bump the patch version from 0.1.7 to 0.1.8 in a separate local commit after the bootstrap fix.

## Authority / Revision

User implementation request 2026-09-28 / r1

## Status

Done

## Depends On

T-0090

## Route / Risk

Quick change / Low

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Updated Cargo.toml and the root dcc package in Cargo.lock to 0.1.8 after fix commit 2f33cb3. Locked Cargo metadata, format, check, all-target Clippy, 591 unit and 101 runnable integration tests (38 ignored as designed), build, CLI version output, release-workflow contract, and diff review passed. No push, tag, or release was performed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0091 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
