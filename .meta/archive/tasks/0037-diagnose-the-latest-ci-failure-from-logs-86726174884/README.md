# T-0037: Diagnose the latest CI failure from `logs_86726174884.zip` and fix the underlying issue.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0037

## Outcome

Diagnose the latest CI failure from `logs_86726174884.zip` and fix the underlying issue.

## Authority / Revision

User request 2026-08-17 / r1

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

CI logs showed the Docker smoke job had one remaining failure: `durable_and_one_shot_container_modes_behave_differently` observed a one-shot `dcc exec` container still running immediately after the command returned. Fixed foreground execution to track whether the invocation created the container, wait only for newly created one-shot containers, and use the same label-based running-container query as reuse detection and the smoke assertion. Added Docker-free unit coverage for the teardown-wait predicate. Verified `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (482 unit, 29 CLI with 3 ignored, 9 config, 33 Docker smokes ignored, 6 feature), `cargo build`, and `git diff --check`. Exact ignored Docker smokes were not runnable locally because `docker` is not installed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0037 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
