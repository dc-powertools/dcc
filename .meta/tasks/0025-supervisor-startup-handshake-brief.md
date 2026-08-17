# T-0025 Brief: Supervisor-Owned Startup, Lifecycle Hooks, and Readiness Handshake

## Identity And Source

- Task ID: T-0025
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User direction following the T-0024 startup-grace defect
- Source reference and date: Startup fragility discussion, 2026-08-13
- Parent or split task IDs: None (follows T-0024)

## Goal

Container startup is coordinated by the in-container supervisor rather than by
host-side polling. The supervisor owns runtime lifecycle hook execution and signals
readiness explicitly; the command harness waits on that signal before running the user's
command. The time-based startup grace period is deleted entirely, because the
drain race it papered over becomes structurally impossible.

## Background

T-0024 moved lifecycle ownership (mode, active-command set, teardown) into a PID 1
supervisor, but left startup sequencing on the host. Today `dcc exec` does:

1. `docker run -d --entrypoint dcc-supervisor …`
2. Poll `docker inspect` until the container reports running (`wait_for_running`,
   `src/exec.rs`, 100 ms interval, 10 s timeout).
3. `docker exec` each `postStartCommand` hook (feature hooks first, then the
   devcontainer hook) via `run_runtime_hooks` / `lifecycle::run_in_container`.
4. `docker exec dcc-exec <user command>`.

Three host-driven round trips after launch, each a failure point. Between step 1 and
step 4 the supervisor sees an empty active-command set, so without mitigation a one-shot
container would drain and exit before the user's command ever arrived. T-0024 mitigated
this with a 60-second time-based startup grace (`STARTUP_GRACE_SECS` in
`src/supervisor.rs`): the supervisor refuses to drain-exit until either a command
registers (`PRIMED`) or the grace elapses.

That grace is fragile in both directions and has already produced one defect: because
`dcc start` intentionally never runs a command, `PRIMED` was never set and a graceful
`dcc stop` inside the grace window was ignored (fixed in T-0024 by letting `STOPPING`
short-circuit the grace, but the underlying time-based design remains). The window it
must cover is also unbounded in practice, because step 3 runs arbitrary user code — a
`postStartCommand` of `npm ci` can exceed any fixed grace.

There is a second, related gap: nothing synchronizes "hooks finished" with "run the
command" other than the host performing them sequentially. `wait_for_running` proves only
that the container process is up, not that it is ready.

## Design

### Approach: hybrid, host-launched with an in-container readiness handshake

The user command remains a `docker exec` — it needs a per-invocation TTY, live streaming
stdio, and a true exit code, none of which survive being run as PID 1 (see Constraints).
What moves into the container is hook execution and readiness sequencing.

**Launch (ephemeral / one-shot mode).** The CLI starts the container with a flag telling
the supervisor to expect an incoming command:

```
docker run -d --entrypoint <rt>/dcc-supervisor <image> --mode oneshot --expect-command
```

`--expect-command` makes the supervisor treat the container as having pending work from
the instant it starts, so the active set is never transiently empty and no time-based
grace is required. `--mode durable` (or `dcc start`) omits `--expect-command`.

Entrypoint args are viable today: `--entrypoint` sets the executable and every argument
after the image tag is passed to it. Mode currently travels as `-e DCC_MODE`; moving it
to an explicit flag alongside `--expect-command` is part of this task.

**Startup sequence.**

1. CLI: `docker run` with `--mode` and, for ephemeral, `--expect-command`.
2. Supervisor (PID 1): performs bootstrapping, then runs `postStartCommand` hooks
   (feature hooks first in installation order, then the devcontainer hook), then marks
   itself ready.
3. CLI: as soon as the container is up, `docker exec`s the command harness
   (`dcc-exec`) with the user's command. It does **not** wait for hooks.
4. Command harness (`dcc-exec`, inside the container):
   a. increments the running count (registers its active-command record),
   b. blocks until the supervisor signals readiness, by successfully passing a
      `dcc-ctl wait-ready` probe,
   c. only then executes the passed-in command,
   d. deregisters on exit (existing `EXIT` trap behavior), propagating the exit code.

The harness registering *before* waiting is what makes the ordering safe: the active set
is non-empty for the entire wait, so the supervisor cannot drain out from under a command
that is queued but not yet running.

**Readiness probe.** `dcc-ctl wait-ready` blocks until the supervisor's readiness marker
exists and exits non-zero (or times out) if the supervisor failed during hooks, so a hook
failure surfaces as a command failure rather than a hang.

**Grace removal.** With `--expect-command` pre-registering pending work and the harness
registering before waiting, `STARTUP_GRACE_SECS` and the `PRIMED` marker are deleted.
`should_exit` reduces to: drain when the active set is empty and (mode is one-shot or a
stop was requested).

### Remaining implementer decisions

- Whether the readiness marker is a file in `/run/dcc` (consistent with existing state) or
  a different mechanism; how `wait-ready` distinguishes "not ready yet" from "startup
  failed" and what its timeout is.
- How hook definitions reach the supervisor: entrypoint args, an env var, or a generated
  manifest file baked/mounted alongside the existing hook assets. Feature hooks are
  already materialized under `/usr/local/share/dcc/`; the devcontainer hook is currently a
  host-side config value.
- Where `${containerEnv:…}` substitution happens for hooks now that they run in-container.
  Today the host resolves it using values probed from the image
  (`docker::inspect_image_env` / `probe_user_env`); running in-container may allow direct
  resolution, but the host/localEnv half of substitution must still happen host-side.
- How `--skip-lifecycle` is conveyed (flag to the supervisor) and how skip warnings still
  reach the host's stderr.
- How hook failure output is surfaced to the host, given hooks no longer run under a
  host-attached `docker exec`.
- Whether `postAttachCommand` also moves. It is per-invocation rather than per-container
  (it runs on every `dcc attach`, not only on container start), so it may need to stay
  host-driven or become a distinct harness step.
- Whether `wait_for_running`'s `docker inspect` polling can be dropped entirely once the
  harness owns the readiness wait, or is still needed to detect a container that failed
  to start at all.

## Scope

In scope:

- `--mode` and `--expect-command` entrypoint arguments for the supervisor; retire
  `DCC_MODE` env-based mode passing.
- Supervisor-side execution of `postStartCommand` hooks (feature + devcontainer).
- A supervisor readiness marker and a `dcc-ctl wait-ready` probe.
- `dcc-exec` harness changes: register, wait for readiness, then exec the command.
- Deletion of `STARTUP_GRACE_SECS`, the `PRIMED` marker, and the grace branch of
  `should_exit`.
- Corresponding host-side changes in `src/exec.rs` (launch args, hook orchestration
  removal or relocation, readiness handling) and `src/build.rs` (build-prep container
  launch stays consistent).
- Hook failure and `--skip-lifecycle` reporting paths.
- Tests: supervisor/harness unit coverage plus ignored Docker smoke coverage for the new
  startup ordering.
- Documentation updates for the revised startup sequence.

Out of scope:

- Running the user's foreground command as PID 1 or via `docker attach`. The command
  stays a `docker exec`.
- Build-preparation hooks (`onCreateCommand`, `updateContentCommand`,
  `postCreateCommand`). They run in the build-prep container during `dcc build` and are
  unaffected.
- The `/cache` mount, state seeding, and the T-0022 root-owned-state friction.
- Any change to the durable/one-shot semantics themselves.

## Users And Workflows

| User/Actor | Workflow | Expected Change |
| --- | --- | --- |
| Developer | `dcc exec`, `dcc run`, `dcc attach` on a cold container | Same observable behavior; startup is less racy and hook failures surface earlier |
| Developer | `dcc start` then `dcc stop` immediately | Works regardless of elapsed time; no grace-window dependency |
| Developer with a slow `postStartCommand` | First command on a cold container | Command waits for hooks to finish rather than racing them; no grace timeout risk |
| Developer using `--skip-lifecycle` | `dcc exec --skip-lifecycle` | Hooks still skipped, warnings still shown on host stderr |
| Maintainer | Reads startup code | One coordinated in-container sequence instead of three host round trips |

## Acceptance Criteria

- [ ] The supervisor accepts `--mode oneshot|durable` and `--expect-command` as
      entrypoint arguments; `DCC_MODE` env passing is removed.
- [ ] `postStartCommand` hooks (feature hooks first in installation order, then the
      devcontainer hook) run inside the supervisor at startup.
- [ ] The supervisor exposes a readiness signal; `dcc-ctl wait-ready` blocks until ready
      and fails (rather than hanging indefinitely) if startup failed.
- [ ] `dcc-exec` registers its active-command record, waits for readiness, then executes
      the command, and still propagates the command's exit code exactly.
- [ ] `STARTUP_GRACE_SECS`, the `PRIMED` marker, and the grace branch of `should_exit`
      are deleted.
- [ ] A one-shot container never drains while a command is registered-but-waiting.
- [ ] A one-shot container with a long-running `postStartCommand` (longer than the old
      60 s grace) completes its command successfully.
- [ ] `dcc start` followed immediately by `dcc stop` tears down correctly.
- [ ] A failing `postStartCommand` surfaces as a clear host-side error, not a hang.
- [ ] `--skip-lifecycle` still skips runtime hooks and still emits warnings on host
      stderr.
- [ ] Durable vs one-shot reuse, `--keep` promotion, and all three `dcc stop` variants
      behave as before.
- [ ] README and `.meta/project/architecture.md` describe the revised startup sequence.
- [ ] Required checks pass: `cargo fmt --check`, `cargo check`,
      `cargo clippy -- -D warnings`, `cargo test`, `cargo build`.

## Constraints

- The user's foreground command must remain a `docker exec` with inherited stdio: it
  requires a per-invocation pseudo-TTY (allocated only when host stdin is a terminal),
  live streaming, correct signal/job-control behavior, and a true exit code returned to
  `dcc`'s own exit status. Running it as PID 1 or via `docker attach` would regress all
  of these and would break concurrent commands in a durable container.
- The supervisor must stay portable across the base images `dcc` targets (glibc,
  BusyBox, Alpine), matching the existing POSIX `sh` bar.
- The supervisor must never exit while any command is registered, including one that is
  registered but still waiting on readiness.
- Keep the read-only `rt` bind mount model: supervisor assets are not writable from
  inside the container.
- Docker-dependent tests are `#[ignore]` and run in CI only
  (`.meta/project/standards.md`).
- `anyhow::Result` with `.with_context`; no `unwrap`/`expect` outside `#[cfg(test)]`.

## Workflow Route Rationale

- Cataloged route and risk: See this task's catalog row.
- Why this route: Restructures container startup sequencing and moves hook execution
  across the host/container boundary, touching every runtime entrypoint.
- Why this risk gate: An error breaks cold-start for all runtime commands, or silently
  skips lifecycle hooks; needs design review and live Docker evidence rather than unit
  tests alone.
- Upstream artifacts required: `.meta/tasks/0024-supervisor-lifecycle-ownership-brief.md`,
  `.meta/tasks/0024-r1-supervisor-design.md`.
- Escalation trigger: If in-container `${containerEnv:…}` substitution cannot reproduce
  the host's current resolution semantics for hooks, escalate before changing hook
  substitution behavior — that is a user-visible config-compatibility decision.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Harness waits on readiness that never arrives | Command hangs instead of failing | `wait-ready` must have a bounded wait and a distinct failure exit; test the failing-hook path |
| Supervisor drains while a command is registered-but-waiting | User command killed before it runs | Register before waiting; explicit test for the queued-command case |
| Hook substitution semantics drift when hooks move in-container | Configs that worked now fail or resolve differently | Keep host/localEnv substitution host-side; test `${containerEnv:…}` hooks end to end |
| Hook output/failures become invisible to the host | Silent lifecycle breakage | Define the reporting path explicitly; smoke-test a failing hook |
| `postAttachCommand` semantics change (per-invocation vs per-start) | Attach hooks run at wrong time or twice | Decide its placement explicitly; existing `lifecycle_hooks_run_in_expected_phases` smoke test guards phase ordering |
| Build-prep container diverges from runtime launch | Inconsistent PID 1 behavior across paths | Update both launch sites together, as in T-0024 |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| Entrypoint args reach the supervisor (args after the image tag are passed to `--entrypoint`) | High | Documented Docker behavior; confirm in a smoke test |
| Feature hook assets are already present in the image for the supervisor to run | Medium | They are generated under `/usr/local/share/dcc/`, but the fast path skips the dcc build stage — verify hook availability there |
| Hooks do not need host-attached stdio | High | They already run non-interactively via `docker exec` with no TTY |
| A readiness marker in the `/run/dcc` tmpfs is sufficient | High | Same mechanism as existing supervisor state |

## Verification Plan

- Automated checks: `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`,
  `cargo test`, `cargo build`.
- Unit tests: supervisor argument parsing (`--mode`, `--expect-command`), readiness
  marker and `wait-ready` semantics, harness register-then-wait ordering, `should_exit`
  with the grace removed.
- Ignored Docker smoke tests: cold-start `dcc exec` ordering; a `postStartCommand`
  longer than the old 60 s grace; a failing `postStartCommand` producing a clear error;
  `dcc start` immediately followed by each `dcc stop` variant; durable reuse and
  `--keep` promotion; hook phase ordering unchanged; `--skip-lifecycle` behavior.
- Manual checks: `dcc --debug exec` shows the new launch args and readiness wait.
- Documentation checks: README and architecture describe the revised sequence.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r1 | 2026-08-13 | User direction | Initial intake | — | — |
| r2 | 2026-08-13 | Design pass + review feedback | Recorded the r3 design (`.meta/tasks/0025-r1-startup-handshake-design.md`) settling all seven implementer decisions; readiness signalling is a single `bootstrap-status` file with a per-waiter FIFO handshake rather than a polled marker; the startup grace is replaced by a supervisor-local `arrived` flag plus a 10 s one-shot orphan reaper; removal of the `image` fast path was split out as T-0027 | The design pass changed the readiness mechanism and added a bounded reaper; the fast-path removal is a user-visible behavior change that needs its own authority | Acceptance criterion "`STARTUP_GRACE_SECS`, the `PRIMED` marker, and the grace branch of `should_exit` are deleted" still holds. The `wait-ready` criterion is met by a FIFO block rather than a poll. A 10 s reaper is added, scoped to one-shot containers and clocked from bootstrap completion, so it cannot bound hook duration. Fast-path removal is out of scope for T-0025 and not a prerequisite |

## Done When

- The supervisor owns startup sequencing and runtime hook execution.
- The command harness registers, waits for readiness, then runs the command.
- The time-based startup grace is gone and cold-start is not time-dependent.
- Hook failures and `--skip-lifecycle` remain visible to the user.
- Required checks pass and documentation matches the implementation.
