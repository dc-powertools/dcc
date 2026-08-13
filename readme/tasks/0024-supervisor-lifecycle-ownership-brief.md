# T-0024 Brief: In-Container Supervisor Lifecycle Ownership

## Identity And Source

- Task ID: T-0024
- Initial revision: r1
- Catalog: `readme/tasks/README.md`
- Accepted source: Architecture decision following the T-0023 container-writable
  bookkeeping review
- Source reference and date: Supervisor-ownership design discussion, 2026-07-21
- Parent or split task IDs: None

## Goal

Container lifecycle authority — durable vs one-shot mode, the set of active commands,
the teardown decision, and container-side lifecycle shutdown — is owned by a single
in-container supervisor process running as PID 1. The host-side file-based runtime
bookkeeping (`src/runtime.rs`) and the `mkdir` lock are removed entirely. `dcc stop`
gains graceful, forceful, and kill variants. The trust boundary between host and
container no longer depends on protecting host-side control-plane files from
container-side writes, eliminating the exposure class that T-0023 addresses.

## Background

`dcc` currently manages container lifecycle from the host. `RuntimeState`
(`src/runtime.rs`) roots at `<workspace>/.dcc/<profile>/runtime/` and holds a mode file,
per-command `.active` records, and a `mkdir`-based lock. Each `dcc run` invocation is a
short-lived host process that creates an active record, acquires the lock, checks for a
running container, runs one command via `docker exec`, then under the lock removes its
record and — in one-shot mode with a drained set — calls `docker stop` and `clear()`s
the directory.

The container's PID 1 is `tail -f /dev/null` (`src/exec.rs` runtime launch and
`build_prep_container_args` in `src/build.rs`), a placeholder keep-alive with no
supervisor logic. The generated `dcc-controller` asset is `exec tail -f /dev/null`
(`src/features/mod.rs`).

Because the bookkeeping lives under the cache root, which is bind-mounted into the
container as read-write `/cache`, container-side code can reach and corrupt it — the
exposure T-0023 was opened to contain. This task removes the exposure by relocating
lifecycle ownership into the container's PID 1 supervisor, so there is no host-trusted
control-plane store for container-side code to reach.

## Design

### Actors

| Actor | Responsibility |
| --- | --- |
| Container supervisor (PID 1) | Owns mode, the active-command set, the teardown decision, and container-side lifecycle shutdown. Single authority for all lifecycle state, held in process memory. |
| Command entrypoint (`docker exec`) | Mediates between the host CLI and the supervisor: registers a command on start, deregisters on exit, relays control messages. Manages one command's lifecycle. Not an authority. |
| Host CLI (`dcc`) | Communicates user intent (`run`, `start`, `stop`) and launches the container when it is not running. Owns no lifecycle state. |

### Teardown and stop semantics

- **One-shot mode:** the supervisor exits when the active set drains and no graceful-stop
  has been requested. PID 1 exiting stops the container (Docker `--rm`).
- **Durable mode:** the supervisor stays alive when the set drains.
- **`dcc stop` (default, graceful):** signals the supervisor to stop accepting new
  commands and exit after all remaining commands finish.
- **`dcc stop --now`:** force-terminates running commands, then runs container lifecycle
  shutdown hooks, then exits.
- **`dcc stop --kill`:** unconditional `docker kill`. Emergency path for wedged or
  corrupted containers.

### Accepted tradeoffs

- Failures that cannot escape the container are not hardened against. A compromised
  dependency or hostile Feature that seizes PID 1 can misbehave within the container but
  cannot reach the host; remediation is `docker kill`.
- Anomalous states are torn down, not corrected. PID-1 exit or `docker kill` is the
  recovery path; no stale-record repair.
- Transient races at teardown and launch boundaries are permitted to fail rather than be
  prevented with locking. A `dcc run` that arrives as the supervisor is exiting, or two
  simultaneous launches, fail with a clear, retryable error.

### Required correctness

The supervisor's drain-vs-registration ordering must be correct: deregistration happens
only on command exit, the set is considered drained only when actually empty, and a
graceful-stop signal refuses new registrations while allowing in-flight commands to
finish. Killing a running user command is not a tolerable edge-case failure.

### Portability

The supervisor must be portable across the base images `dcc` supports. The current
`tail -f /dev/null` keep-alive was chosen for glibc/BusyBox/Alpine portability; the
supervisor must meet the same bar. T-0024-R1 determines whether a POSIX shell script or a
small static binary is appropriate.

## Scope

In scope:

- A PID 1 supervisor that owns mode, the active-command set, teardown, and shutdown hooks.
- A command entrypoint that mediates between the host CLI and the supervisor.
- Removal of `src/runtime.rs` (`RuntimeState`, `ContainerMode`, `ActiveCommand`,
  `RuntimeLock`) and all host-side bookkeeping file I/O.
- Reworked `dcc run`, `dcc exec`, `dcc attach`, `dcc start`, and `dcc stop` to drive the
  supervisor through the entrypoint instead of managing host-side files.
- `dcc stop` graceful / `--now` / `--kill` variants.
- Update the build-preparation container (`src/build.rs`) to use the supervisor as PID 1.
- Update the generated `dcc-controller` / `dcc-command` assets (`src/features/mod.rs`).
- Update `readme/threat-models/0004-dcc-runtime.md`, README, and
  `readme/project/architecture.md`.
- Tests covering the supervisor, the entrypoint mediation, the stop variants, and the
  removed host-side bookkeeping.

Out of scope:

- T-0021 state-path guards and T-0022 state seeding (independent; land on their own).
- T-0023 containment (this task supersedes it — see Relationship To Other Tasks).
- Any broader container-escape hardening or sandboxing model.
- A host-side supervisor or daemon; the host CLI remains a short-lived process.

## Relationship To Other Tasks

T-0023 (contain host-side bookkeeping from the `/cache` mount) is **superseded** by this
task. T-0024 removes the host-side bookkeeping entirely, so there is nothing to contain.
T-0023 should be closed as superseded when T-0024 is accepted, with a note pointing here.
If T-0024 is rejected or descoped, T-0023 remains the fallback containment task.

T-0021 and T-0022 are independent. T-0024 must stay consistent with T-0022's
`.dcc/<profile>.seed.json` sibling-file convention if T-0022 lands first, but does not
depend on it.

## Sub-Tasks

| ID | Outcome | Depends On | Route / Risk |
| --- | --- | --- | --- |
| T-0024-R1 | Research and design the supervisor: choose implementation language (POSIX shell vs static binary), define the CLI↔entrypoint↔supervisor message protocol, specify the active-set and mode state machine, and specify the `stop` graceful/`--now`/`--kill` semantics. Produce a design document for review. | None | Design / High |
| T-0024-S1 | Implement the PID 1 supervisor and the command entrypoint, including the active-set state machine, mode handling, graceful-stop gating, and `--now` shutdown-hook execution. Generate them as in-container assets. | T-0024-R1 | Initiative / High |
| T-0024-H1 | Rework the host CLI (`src/exec.rs`, `src/stop.rs`, `src/docker.rs`) to drive the supervisor through the entrypoint; remove `src/runtime.rs` and all host-side bookkeeping file I/O; add `dcc stop` graceful / `--now` / `--kill` variants. | T-0024-S1 | Initiative / High |
| T-0024-B1 | Update the build-preparation container (`src/build.rs`) and generated controller/command assets (`src/features/mod.rs`) to use the supervisor as PID 1. | T-0024-S1 | Quick change / Medium |
| T-0024-T1 | Add tests: supervisor state-machine unit tests, entrypoint mediation tests, `stop` variant behavior, and ignored Docker smoke tests asserting no host-side bookkeeping exists and teardown/reuse/stop behave correctly. | T-0024-H1, T-0024-B1 | Quick change / Medium |
| T-0024-D1 | Update `readme/threat-models/0004-dcc-runtime.md`, README, and `readme/project/architecture.md` to reflect supervisor ownership and the removed trust boundary; close T-0023 as superseded. | T-0024-H1 | Quick change / Low |

## Acceptance Criteria

- [ ] A PID 1 supervisor owns container mode, the active-command set, the teardown
      decision, and container-side shutdown hooks.
- [ ] The command entrypoint mediates between the host CLI and the supervisor for command
      registration/deregistration and control signals.
- [ ] `src/runtime.rs` and all host-side bookkeeping file I/O are removed; no
      `<workspace>/.dcc/<profile>/runtime/` directory is created.
- [ ] One-shot containers stop automatically when the last command exits; durable
      containers stay alive.
- [ ] `dcc stop` (default) drains: the supervisor stops accepting new commands and exits
      after remaining commands finish.
- [ ] `dcc stop --now` force-terminates running commands, runs shutdown hooks, and exits.
- [ ] `dcc stop --kill` unconditionally `docker kill`s the container.
- [ ] The supervisor never exits while a registered command is still running.
- [ ] A `dcc run` arriving during teardown fails with a clear, retryable error.
- [ ] `${containerCacheFolder}` / `/cache` remains usable for user cache data.
- [ ] The supervisor is portable across glibc, BusyBox, and Alpine base images.
- [ ] The build-preparation container uses the supervisor as PID 1.
- [ ] `readme/threat-models/0004-dcc-runtime.md` records the supervisor ownership model
      and the removed trust boundary; T-0023 is closed as superseded.
- [ ] README and `readme/project/architecture.md` match the implemented design.
- [ ] Required checks pass: `cargo fmt --check`, `cargo check`, `cargo clippy -- -D
      warnings`, `cargo test`, `cargo build`.

## Constraints

- Do not regress the existing `--tmpfs /workspace/.dcc` masking.
- Stay independent of T-0021 and T-0022.
- Docker-dependent tests are `#[ignore]` and run in CI only
  (`readme/project/standards.md`).
- `anyhow::Result` with `.with_context`; no `unwrap`/`expect` outside `#[cfg(test)]`.
- The supervisor must not kill a registered, running command except via `dcc stop --now`
  or `--kill`.

## Workflow Route Rationale

- Cataloged route and risk: See this task's catalog row.
- Why this route: A security-motivated rearchitecture of the lifecycle authority split,
  touching every runtime command and the container's PID 1 contract. High risk because an
  error breaks container reuse, teardown, or stop for all users.
- Why this risk gate: The change is motivated by a security finding and restructures a
  trust boundary, so it needs a design phase (T-0024-R1), threat-model evidence, and
  review rather than tests alone.
- Upstream artifacts required: `readme/threat-models/0004-dcc-runtime.md`.
- Escalation trigger: If the design phase finds that the supervisor cannot be made
  portable across supported base images without a static binary, escalate the
  distribution story before implementing.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Supervisor exits early, killing a running command | User data loss, broken workflows | Drain-vs-registration ordering must be correct; dedicated tests |
| Supervisor not portable across base images | Broken containers on Alpine/BusyBox | T-0024-R1 validates portability before implementation |
| `docker exec` message delivery is unreliable | Orphaned active records, premature or missed teardown | Deregistration on command exit is the source of truth; anomalous state torn down via `--kill` |
| Removing host-side bookkeeping breaks `dcc stop` for already-running legacy containers | Stale containers from before the migration | `dcc stop --kill` handles any container regardless of bookkeeping |
| Build-prep and runtime paths diverge on supervisor wiring | Inconsistent behavior | T-0024-B1 covers both; shared asset generation |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| No host-side process needs to read lifecycle state independently | High | The CLI's reuse check can rely on Docker's running-container query alone |
| PID 1 can reliably detect command exit for deregistration | High | The entrypoint is the command's parent process; deregistration is its last act |
| Docker `--rm` stops the container when PID 1 exits | High | Documented Docker behavior; confirmed by smoke tests |

## Verification Plan

- Automated checks: `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`,
  `cargo test`, `cargo build`.
- Unit tests: supervisor state machine, entrypoint mediation, stop-variant semantics,
  mount-argument construction.
- Ignored Docker smoke tests: one-shot auto-stop on drain; durable survival; `dcc stop`
  graceful drain; `dcc stop --now` force + shutdown hooks; `dcc stop --kill`; reuse
  across invocations; teardown-race failure is clear and retryable; no
  `<workspace>/.dcc/<profile>/runtime/` directory is created.
- Manual checks: `dcc --debug run` shows the supervisor as PID 1; `dcc stop` variants
  behave as specified.
- Documentation checks: threat model, README, and architecture updated consistently.
- Portability checks: supervisor runs under glibc, BusyBox, and Alpine base images in CI.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r1 | 2026-07-21 | Architecture decision | Initial intake | — | — |

## Done When

- A PID 1 supervisor owns the container lifecycle; no host-side bookkeeping remains.
- `dcc stop` graceful / `--now` / `--kill` variants behave as specified.
- One-shot, durable, reuse, and teardown behave correctly with observed evidence.
- The supervisor never kills a running command except via `--now` / `--kill`.
- The threat model records the new trust boundary; T-0023 is closed as superseded.
- Required checks pass and documentation matches the implementation.
