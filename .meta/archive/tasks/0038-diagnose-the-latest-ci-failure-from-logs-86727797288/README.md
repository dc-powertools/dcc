# T-0038: Diagnose the latest CI failure from `logs_86727797288.zip` and fix the underlying issue.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0038

## Outcome

Diagnose the latest CI failure from `logs_86727797288.zip` and fix the underlying issue.

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

CI logs showed `durable_and_one_shot_container_modes_behave_differently` still failed after the host-side teardown wait: a very fast one-shot command could register and deregister entirely between supervisor polling intervals, so PID 1 never observed `active_count > 0`, left `arrived=0`, and waited for the 10 s orphan reaper. Fixed `dcc-exec` to write a persistent `$STATE/arrived` marker after creating the active record, and taught the supervisor to treat that marker as command arrival. Added a real `/bin/sh` regression test that runs the generated supervisor and `dcc-exec /bin/true` together and asserts one-shot drain without waiting for the orphan reaper. Verified focused supervisor tests, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (483 unit, 29 CLI with 3 ignored, 9 config, 33 Docker smokes ignored, 6 feature), `cargo build`, and `git diff --check`. Exact ignored Docker smokes were not runnable locally because `docker` is not installed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0038 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
