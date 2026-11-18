# T-0015: Update user-facing documentation to match the completed `dcc` rewrite design and implementation.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0015

## Outcome

Update user-facing documentation to match the completed `dcc` rewrite design and implementation.

## Authority / Revision

User request 2026-07-20 / r1

## Status

Done

## Depends On

T-0004, T-0013, T-0014

## Route / Risk

Quick change / Low

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Reworked the README around the current mental model, common workflows, `customizations.dcc` config, explicit build requirements, named `dcc run` commands versus direct `dcc exec`, one-shot/durable reuse, dry-run/JSON output, and documented differences from normal devcontainers including ignored compatibility fields, port forwarding, unsafe runtime gating, and unsupported top-level `remoteUser`. Verified `git diff --check`, stale README searches, and `cargo test`.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0015 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
