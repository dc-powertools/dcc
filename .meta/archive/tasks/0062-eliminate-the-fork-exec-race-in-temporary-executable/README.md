# T-0062: Eliminate the fork/exec race in temporary executable test fixtures and make release-gating reusable CI check the requested tag ref.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0062

## Outcome

Eliminate the fork/exec race in temporary executable test fixtures and make release-gating reusable CI check the requested tag ref.

## Authority / Revision

User request 2026-08-24 / r1

## Status

Done

## Depends On

T-0061

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Executable fixtures are now written entirely by a short-lived child process, so the parallel Rust test process never owns a writable descriptor that another fork can inherit; a bounded process-churn regression protects the boundary. Reusable CI accepts an optional `ref`, all five jobs check out that input or the triggering SHA, and release forwards its requested tag into CI. The 12 forwarding tests passed, the churn regression passed 50 consecutive runs, `actionlint 1.7.12` accepted every workflow, and format, check, Clippy, the full test suite (511 unit plus all runnable integration tests), build, and diff checks passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [threat ci ref and fixture](threat-ci-ref-and-fixture.md) — Security and trust-boundary analysis.

Additional original references: `.meta/archive/tasks/0062-eliminate-the-fork-exec-race-in-temporary-executable/threat-ci-ref-and-fixture.md`.

## Provenance

Migrated from the T-0062 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
