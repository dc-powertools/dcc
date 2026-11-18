# T-0045: Make `updateRemoteUserUID` behavior Linux-only as specified and add host-independent tests for Linux and non-Linux platform decisions.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0045

## Outcome

Make `updateRemoteUserUID` behavior Linux-only as specified and add host-independent tests for Linux and non-Linux platform decisions.

## Authority / Revision

Parent T-0043; user follow-up 2026-08-24 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Replaced the Unix-wide gate with an injected Linux platform decision and restricted `host_ids()` to `target_os = "linux"`. Deterministic tests now prove simulated Linux remaps and simulated macOS/Windows no-ops, while disabled/root/numeric/unavailable-ID and in-image collision/already-matching coverage remains. Replaced the no-panic host smoke with observed numeric UID/GID assertions and reconciled architecture, T-0026 design, and quality evidence. Focused 13 tests, format, clippy, full test (492 unit plus integration suites), build, and diff review passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [uid platform contract brief](uid-platform-contract-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0045-uid-platform-contract/uid-platform-contract-brief.md`.

## Provenance

Migrated from the T-0045 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
