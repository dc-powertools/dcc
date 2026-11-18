# T-0024: Move container lifecycle ownership into an in-container PID 1 supervisor; remove host-side file-based runtime bookkeeping and the `mkdir` lock; add `dcc stop` graceful / `--now` / `--kill` variants.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0024

## Outcome

Move container lifecycle ownership into an in-container PID 1 supervisor; remove host-side file-based runtime bookkeeping and the `mkdir` lock; add `dcc stop` graceful / `--now` / `--kill` variants.

## Authority / Revision

Architecture decision 2026-07-21 / r1

## Status

Done

## Depends On

None

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Implemented in-container PID 1 lifecycle supervisor (`src/supervisor.rs`): POSIX sh scripts bind-mounted read-only from `.dcc/<profile>.rt/`; state in tmpfs `/run/dcc`; one-shot self-teardown on drain; `dcc stop` graceful/--now/--kill. Removed `src/runtime.rs`; reworked exec.rs, stop.rs, build.rs; removed dead dcc-controller/dcc-command assets; added docker::kill_container. Updated threat model, README, architecture. T-0023 superseded. Verified fmt, clippy (0 prod warnings), 506 tests, build.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [supervisor lifecycle ownership brief](supervisor-lifecycle-ownership-brief.md) — Scope, design or execution evidence.
- [r1 supervisor design](../0024-r1-r1-supervisor-design/r1-supervisor-design.md) — Scope, design or execution evidence; canonical owner T-0024-R1.

Additional original references: `.meta/archive/tasks/0024-supervisor-lifecycle-ownership/supervisor-lifecycle-ownership-brief.md`; `.meta/archive/tasks/0024-r1-r1-supervisor-design/r1-supervisor-design.md`.

## Provenance

Migrated from the T-0024 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
