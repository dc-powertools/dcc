# T-0088: Make `ghcr.io/dc-powertools/features` the built-in default repository for short Feature names while retaining project overrides and declaration-scoped inheritance.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0088

## Outcome

Make `ghcr.io/dc-powertools/features` the built-in default repository for short Feature names while retaining project overrides and declaration-scoped inheritance.

## Authority / Revision

User amendment 2026-09-09 / r1

## Status

Done

## Depends On

T-0087

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added the built-in `ghcr.io/dc-powertools/features` fallback to the shared project Feature normalizer, so unconfigured and root-null short names resolve consistently in config and `dcc feature`. Configured/inherited overrides, parent provenance, canonical collisions, explicit/local references, and explicit-only Feature dependencies remain intact. The absent-setting counterfactual failed against the prior implementation; format, check, all-target Clippy, 591 unit tests plus 93 runnable integration tests, build, diff, and focused contract/documentation review passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [built in default feature repository notes](built-in-default-feature-repository-notes.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0088-built-in-default-feature-repository/built-in-default-feature-repository-notes.md`.

## Provenance

Migrated from the T-0088 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
