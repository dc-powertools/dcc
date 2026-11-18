# T-0024-H1: Rework the host CLI (`src/exec.rs`, `src/stop.rs`, `src/docker.rs`) to drive the supervisor through the entrypoint; remove `src/runtime.rs` and all host-side bookkeeping file I/O; add `dcc stop` graceful / `--now` / `--kill` variants.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0024-H1

## Outcome

Rework the host CLI (`src/exec.rs`, `src/stop.rs`, `src/docker.rs`) to drive the supervisor through the entrypoint; remove `src/runtime.rs` and all host-side bookkeeping file I/O; add `dcc stop` graceful / `--now` / `--kill` variants.

## Authority / Revision

Parent T-0024 / r1

## Status

Done

## Depends On

T-0024-S1

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Reworked exec/stop; deleted runtime.rs; added stop variants + kill_container; 3 CLI tests.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0024-H1 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
