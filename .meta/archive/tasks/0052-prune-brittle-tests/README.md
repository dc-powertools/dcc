# T-0052: Remove or rewrite redundant and implementation-coupled tests whose assertions do not protect a stable user, compatibility, or safety property.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0052

## Outcome

Remove or rewrite redundant and implementation-coupled tests whose assertions do not protect a stable user, compatibility, or safety property.

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

Classified every named candidate. Deleted nine duplicate, retired-symbol, exact-layout, or misleading tests; rewrote supervisor assets, root/version-label behavior, and Feature-before-netcat ordering around stable outcomes. Positive stop dry-run and real-shell supervisor tests retain user-observable coverage. Focused modules, format, clippy, full test (495 unit plus integration suites), build, diff check, and coverage mapping passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [prune brittle tests brief](prune-brittle-tests-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0052-prune-brittle-tests/prune-brittle-tests-brief.md`; `.meta/archive/tasks/0043-test-quality-corrections/test-quality-corrections-notes.md#t-0052-candidate-classification`.

## Provenance

Migrated from the T-0052 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
