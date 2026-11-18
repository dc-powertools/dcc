# T-0063: Remove duplicate CI from the autotag-to-release path while preserving one exact-commit CI gate for automatic bumps and an independent CI gate for direct tag pushes.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0063

## Outcome

Remove duplicate CI from the autotag-to-release path while preserving one exact-commit CI gate for automatic bumps and an independent CI gate for direct tag pushes.

## Authority / Revision

User request 2026-08-24 / r1

## Status

Done

## Depends On

T-0062

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

The bump job now exports its newly committed SHA and passes it to autotag; automatic-push autotag uses the triggering SHA. Autotag runs reusable CI and creates the version tag from that same immutable commit, then calls release with `ci_already_passed: true`. Release skips only that redundant CI run; direct tag pushes still run CI, and release builds require either trusted reuse or an explicit successful CI result. Maintainer documentation and the CI threat model reflect the flow. Static ref/gate contract checks, `actionlint 1.7.12` across every workflow, format, check, Clippy, 511 unit plus all runnable integration tests, build, and diff checks passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [threat release ci reuse](threat-release-ci-reuse.md) — Security and trust-boundary analysis.

Additional original references: `.meta/archive/tasks/0063-remove-duplicate-ci-from-the-autotag-to-release/threat-release-ci-reuse.md`.

## Provenance

Migrated from the T-0063 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
