# T-0054: Implement `updateRemoteUserUID` mapping on macOS while preserving the established Linux behavior and explicit Windows no-op.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0054

## Outcome

Implement `updateRemoteUserUID` mapping on macOS while preserving the established Linux behavior and explicit Windows no-op.

## Authority / Revision

User request 2026-08-24 / r1

## Status

Done

## Depends On

T-0026, T-0045

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

macOS now discovers host UID/GID and plans the existing safeguarded build-stage remap; Linux behavior is unchanged, and Windows/unsupported hosts explicitly no-op. Deterministic platform tests, focused UID/Dockerfile tests, format, check, clippy, the full test suite, build, and diff checks passed. A live macOS Docker Desktop run remains environment-bound residual evidence.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality macos uid remap quality](quality-macos-uid-remap-quality.md) — Verification and review evidence.
- [macos uid remap brief](macos-uid-remap-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0054-macos-uid-remap/macos-uid-remap-brief.md`; `.meta/archive/tasks/0054-macos-uid-remap/quality-macos-uid-remap-quality.md`.

## Provenance

Migrated from the T-0054 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
