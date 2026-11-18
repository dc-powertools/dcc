# T-0067: Fix the skipped `Create Release` job while preserving exact-commit CI reuse for automatic releases and CI gating for direct tag pushes.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0067

## Outcome

Fix the skipped `Create Release` job while preserving exact-commit CI reuse for automatic releases and CI gating for direct tag pushes.

## Authority / Revision

User request 2026-08-25 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added an explicit `!cancelled()` plus successful aggregated-build condition to `Create Release`, so intentional reusable-CI skips no longer suppress publication and failed/cancelled builds still cannot publish. Added a CI-enforced release-workflow contract script covering CI reuse, direct-tag gating, build gating, and publication. Counterfactual failure, the corrected contract, Bash syntax, ShellCheck, `actionlint 1.7.12`, format, all-target Clippy, the full test suite, diff, threat-model, and consistency reviews passed. No push or release was triggered; the next authorized release remains the live scheduler confirmation.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [incident skipped release publication](incident-skipped-release-publication.md) — Incident analysis.
- [quality release publication gate quality](quality-release-publication-gate-quality.md) — Verification and review evidence.
- [threat release ci reuse](../0063-remove-duplicate-ci-from-the-autotag-to-release/threat-release-ci-reuse.md) — Security and trust-boundary analysis; canonical owner T-0063.

Additional original references: `.meta/archive/tasks/0067-fix-the-skipped-create-release-job-while-preserving/quality-release-publication-gate-quality.md`; `.meta/archive/tasks/0063-remove-duplicate-ci-from-the-autotag-to-release/threat-release-ci-reuse.md`; `.meta/archive/tasks/0067-fix-the-skipped-create-release-job-while-preserving/incident-skipped-release-publication.md`.

## Provenance

Migrated from the T-0067 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
