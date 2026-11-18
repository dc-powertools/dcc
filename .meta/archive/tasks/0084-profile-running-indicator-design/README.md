# T-0084: Design an extension to `dcc profile list` that indicates whether each discovered profile currently has a running dcc container while preserving trustworthy offline behavior and stable text/JSON output.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0084

## Outcome

Design an extension to `dcc profile list` that indicates whether each discovered profile currently has a running dcc container while preserving trustworthy offline behavior and stable text/JSON output.

## Authority / Revision

User request and square-bracket marker amendment 2026-09-08 / r2

## Status

Done

## Depends On

None

## Route / Risk

Decide / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; implementation-ready design complete.

## Result

Designed a default best-effort running indicator using one batched, read-only `docker ps` label snapshot; visually distinct `[running]` text beside the existing `(default)` label; tri-state JSON; truthful Docker-unavailable degradation; explicit runtime/build-prep role labels with legacy fallback; async enrichment without config parsing; and focused, fake-Docker, and live-Docker verification criteria. A focused review corrected unknown future roles to produce unknown status rather than a false stopped result. No production code changed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [profile running indicator design](profile-running-indicator-design.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0084-profile-running-indicator-design/profile-running-indicator-design.md`.

## Provenance

Migrated from the T-0084 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
