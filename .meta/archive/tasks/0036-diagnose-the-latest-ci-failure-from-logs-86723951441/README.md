# T-0036: Diagnose the latest CI failure from `logs_86723951441.zip` and fix the remaining Docker smoke regressions.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0036

## Outcome

Diagnose the latest CI failure from `logs_86723951441.zip` and fix the remaining Docker smoke regressions.

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

CI logs showed the Docker smoke job now passed the earlier `exec_runs_direct_command` case but failed four smokes: generated startup hooks masked `exit 42` because they relied on non-POSIX `PIPESTATUS`; seeded non-root state remained owned by the pre-remap uid; and one smoke invoked `dcc exec --keep run check` instead of named-command `dcc run --keep check`. Fixed hook status preservation under POSIX `sh`, re-owned hydrated state to non-root `containerUser` after tar extraction, corrected file-state hydration parent creation, updated the durable reuse smoke, and refreshed ownership docs/decision notes. Verified focused hook and seed tests, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (481 unit, 29 CLI with 3 ignored, 9 config, 6 feature; 33 Docker smokes still ignored), `cargo build`, stale-doc searches, and `git diff --check`. Exact ignored Docker smokes were not runnable locally because `docker` is not installed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0036 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
