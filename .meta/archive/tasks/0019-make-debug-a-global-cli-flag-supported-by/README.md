# T-0019: Make `--debug` a global CLI flag supported by all commands.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0019

## Outcome

Make `--debug` a global CLI flag supported by all commands.

## Authority / Revision

User request 2026-07-20 / r1

## Status

Done

## Depends On

T-0018

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Moved `--debug` from runtime subcommands to the global CLI options, threaded it through existing runtime launch debugging, added build/stop/id debug output for resolved profile/config/container/image details, updated README and architecture docs, and added CLI tests for global placement plus non-runtime debug output. Verified fmt, tests, clippy, help output, grep checks, and diff check.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0019 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
