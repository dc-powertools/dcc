# T-0012: Enable Docker-dependent smoke tests in GitHub Actions while keeping local development-container tests Docker-free by default.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0012

## Outcome

Enable Docker-dependent smoke tests in GitHub Actions while keeping local development-container tests Docker-free by default.

## Authority / Revision

User request 2026-07-20 / r1

## Status

Done

## Depends On

T-0011

## Route / Risk

Quick change / Low

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added a `docker-test` CI job on GitHub-hosted Ubuntu runners that verifies Docker availability and explicitly runs the ignored Docker smoke tests. Updated test comments and standards to record that local `cargo test` remains Docker-free while CI owns live Docker build/run coverage. Verified workflow syntax with `actionlint`, listed the ignored Docker tests without running them locally, and passed fmt, clippy, tests, and build.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0012 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
