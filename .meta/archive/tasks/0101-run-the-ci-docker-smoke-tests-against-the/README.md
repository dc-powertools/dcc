# T-0101: Run the CI Docker smoke tests against the current main source.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0101

## Outcome

Run the CI Docker smoke tests against the current main source.

## Authority / Revision

User request 2026-11-16 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / Medium

## Approval Or Blocker

User authorized container launches for these tests, superseding the earlier no-launch restriction for this scope.

## Next Safe Action

Stop; requested test run complete, forwarding qualification still requires Engine 28+.

## Result

Ran all five CI commands with --locked on main 54a8d71 (same product source as fetched origin/main c68b82c): 37 passed, 1 failed. Forwarding test failed on Engine 25.0.18; focused diagnostic confirmed explicit Engine 28+ refusal. No code changed; T-0097 remains Needs verification.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality docker smoke main](quality-docker-smoke-main.md) — Verification and review evidence.

Additional original references: `.meta/archive/tasks/0101-run-the-ci-docker-smoke-tests-against-the/quality-docker-smoke-main.md`.

## Provenance

Migrated from the T-0101 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
