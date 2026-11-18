# T-0007: Update Feature metadata handling for state, commands, unsupported properties, unsafe runtime settings, and lifecycle hook collection order.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0007

## Outcome

Update Feature metadata handling for state, commands, unsupported properties, unsafe runtime settings, and lifecycle hook collection order.

## Authority / Revision

Parent T-0004 / r1

## Status

Done

## Depends On

T-0005, T-0006

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added Feature `customizations.dcc.commands` and `state`, legacy `scripts` warnings, Feature state metadata/runtime merge, unsupported `init`/`entrypoint` warnings, Feature user-property errors, unsafe Feature runtime gating via `--allow-unsafe-runtime`, docs, and tests. Verified fmt, clippy, tests, and build.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [feature metadata brief](feature-metadata-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0007-feature-metadata/feature-metadata-brief.md`.

## Provenance

Migrated from the T-0007 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
