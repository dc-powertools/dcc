# T-0025: Move startup sequencing and runtime lifecycle hooks into the supervisor: `--mode`/`--expect-command` entrypoint args, supervisor-run `postStartCommand`, a `dcc-ctl wait-ready` readiness probe the command harness blocks on after registering, and deletion of the time-based startup grace.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0025

## Outcome

Move startup sequencing and runtime lifecycle hooks into the supervisor: `--mode`/`--expect-command` entrypoint args, supervisor-run `postStartCommand`, a `dcc-ctl wait-ready` readiness probe the command harness blocks on after registering, and deletion of the time-based startup grace.

## Authority / Revision

User direction 2026-08-13 / r2

## Status

Done

## Depends On

T-0024

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Implemented per the r3 design: supervisor accepts `--mode`/`--expect-command`/`--start-hooks` entrypoint args (retired `DCC_MODE` env); runs `postStartCommand` hooks (feature-first, then devcontainer) inside PID 1 from host-generated pre-substituted scripts in `.dcc/<profile>.rt/start-hooks/`; writes a single `/run/dcc/bootstrap-status` file (`0` or `<exit-code> <hook-name>`); `dcc-ctl wait-ready` uses a per-waiter FIFO handshake (mkfifo before status check, `<>` open to avoid wedging on orphaned FIFOs) — no polling, no package dependency; `dcc-exec` registers then waits for readiness before running the command; deleted `STARTUP_GRACE_SECS`/`PRIMED`/grace branch, replaced with a supervisor-local `arrived` variable + 10 s one-shot post-bootstrap orphan reaper; `postAttachCommand` stays host-side with a cold-start `wait-ready`; host maps exit 252 to a clear hook-failure error. `${containerEnv:…}` substitution stays host-side (no config-compatibility change). Updated `src/supervisor.rs` (full rewrite of all three scripts + hook-script generation + `sh_quote` + `RtDir::write_start_hooks`/`has_start_hooks`), `src/exec.rs` (entrypoint args, start-hook emission, `wait_ready`, removed `run_runtime_hooks(Startup)`), `src/build.rs` (`--mode durable` entrypoint arg). 478 unit tests (incl. 6 real-`sh` execution tests for bootstrap/failure/FIFO/fast-path/252), 33 ignored Docker smokes (9 new: cold-start ordering, slow hook, failing hook, start+stop variants, skip-lifecycle, drain, durable reuse). Verified fmt, clippy (0 warnings incl. `--all-targets`), test, build. Live Docker execution delegated to CI. Fast-path removal split out as T-0027 (not a prerequisite).

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [r1 startup handshake design](r1-startup-handshake-design.md) — Scope, design or execution evidence.
- [supervisor startup handshake brief](supervisor-startup-handshake-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0025-supervisor-startup-handshake/supervisor-startup-handshake-brief.md`; `.meta/archive/tasks/0025-supervisor-startup-handshake/r1-startup-handshake-design.md`.

## Provenance

Migrated from the T-0025 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
