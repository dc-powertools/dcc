# T-0025 R1 Design: Supervisor-Owned Startup And Readiness Handshake

Design pass for T-0025. Parent brief:
`readme/tasks/0025-supervisor-startup-handshake-brief.md`. Predecessor:
`readme/tasks/0024-r1-supervisor-design.md`.

This document resolves the seven "remaining implementer decisions" in the brief.
Revision r2 added a single `bootstrap-status` file, a 10-second orphan reaper, and the
removal of all startup sentinel files. Revision r3 makes the in-container readiness wait
event-driven via a per-waiter FIFO, and records the image fast path's removal as a
separate task (T-0027).

## Summary Of Decisions

| # | Question | Decision |
| --- | --- | --- |
| D0 | The image fast path | **Remove it** — it cannot carry dcc's own machinery, and every feature must work around it. Split out as T-0027 |
| D1 | Readiness marker, failure signalling, timeout | Single `/run/dcc/bootstrap-status` file: `0` or `<exit-code> <hook-name>`; waiters block on a per-waiter FIFO, no polling |
| D2 | How hook definitions reach the supervisor | Host-generated executable scripts in `.dcc/<profile>.rt/start-hooks/`, passed via `--start-hooks <dir>` |
| D3 | Where `${containerEnv:…}` substitution happens | Entirely host-side, unchanged. No escalation needed |
| D4 | How `--skip-lifecycle` is conveyed | Not conveyed. The host emits no hook scripts and warns host-side |
| D5 | How hook failure output reaches the host | Hook output tee'd to `/run/dcc/hook.log`; `wait-ready` replays the tail on failure through the harness's inherited stderr |
| D6 | Whether `postAttachCommand` moves | Stays host-side, gated behind a host-side `wait-ready` |
| D7 | Whether `wait_for_running` can be dropped | Kept. It is the only detector of a container that never started |
| D8 | Startup race and orphan containment | Supervisor-local `arrived` variable + 10s post-bootstrap reaper. No sentinel files |

## D0 — Remove The Image Fast Path (tracked as T-0027)

`uses_fast_path` (`src/build.rs`) pulls the user's image and `docker tag`s it *without*
running the dcc build stage, so nothing dcc generates reaches the image. It has been a
standing tax on every feature since:

| Task | Tax paid |
| --- | --- |
| T-0024 | Supervisor could not be baked into the image; forced the read-only `rt` bind-mount design |
| T-0022 | `uses_fast_path` must return false whenever `state` is declared |
| T-0026 | Needed a fast-path-implies-no-remap invariant plus a guard test |
| T-0025 | Blocks installing any package (e.g. `inotify-tools`) the supervisor might need |

It is also load-bearing in `version.rs`, `stop.rs`, `run.rs`, and `exec.rs`, purely to
suppress a version-mismatch warning for images that carry no dcc version stamp.

Per direction: **the fast path should be removed.** Its condition is degenerate anyway — an
`image` source, no `build`, no features, no `containerEnv`, no `forwardPorts`, no
build-prep hooks, no declared state, *and* `containerUser: root`. Such a profile pays one
extra `docker build` of a `FROM <image>` stage with a version stamp, which is
near-instant and cached. In exchange, every container dcc creates is uniformly a dcc
image: the supervisor, hooks, and any future dependency can be baked in, and four call
sites lose a special case.

This is a user-visible behavior change beyond T-0025's original scope, so **it is tracked
separately as catalog task T-0027** rather than folded in here.
`warn_if_image_version_mismatch` loses its `current_uses_fast_path` parameter, since a
missing version stamp becomes unambiguously a stale image.

**T-0027 is not a prerequisite for T-0025.** Nothing in this design depends on the fast
path being gone: hook scripts and the supervisor both ride the `rt` bind mount, and the
FIFO handshake needs no installed package. D0's only role here was to establish that the
fast path is not a reason to reject an approach — it is recorded as a decision, not a
dependency.

Note this does **not** resurrect baking the supervisor into the image. The `rt` bind mount
is retained: it keeps supervisor scripts read-only from inside the container and lets a
`dcc` upgrade fix the supervisor without an image rebuild. D0's value here is removing the
constraint that *forbade* installing anything, which is what makes D1's FIFO viable
without a fallback.

> **Update (T-0028):** this paragraph is superseded by
> `readme/decisions/0004-embed-supervisor-in-image.md`. The supervisor *is* now baked into
> the image; both reasons given above were discarded (tamper-resistance is a declared
> non-goal, and version skew is handled by a semver compatibility gate instead). Startup
> hook scripts stay on the `rt` mount — not for the reasons above, but because
> `${localEnv:VAR}` in `postStartCommand` is only resolvable at run time.

## D1 — Single `bootstrap-status` File, With An Event-Driven Wait

The supervisor's startup phase ends by writing exactly one file:

| Path | Contents |
| --- | --- |
| `/run/dcc/bootstrap-status` | `0` on success, or `<exit-code> <hook-name>` on failure |
| `/run/dcc/hook.log` | Combined stdout/stderr of all startup hooks (read only on failure) |
| `/run/dcc/waiters/<id>` | One FIFO per waiter blocked in `wait-ready` |

One file to watch, one read to interpret.

### The in-container wait is event-driven, via a per-waiter FIFO

The wait that matters is `dcc-exec` (inside the container) waiting for the supervisor to
finish bootstrapping. Polling it was the weak point in r2. It is now event-driven with
**no polling and no package dependency**, using a named pipe per waiter.

`dcc-ctl wait-ready` does, in this exact order:

1. `mkfifo /run/dcc/waiters/<id>` — register interest **first**;
2. check `bootstrap-status`; if it already exists, remove the FIFO and return immediately;
3. otherwise `read` from the FIFO, which blocks in `open(2)` until the supervisor writes;
4. remove the FIFO and read `bootstrap-status`.

Step 1 before step 2 is what makes it lossless: the FIFO exists before the status check,
so a signal arriving in between cannot be missed. Step 2 handles steady state, where
readiness was reached long ago and the waiter must not block at all.

The supervisor, immediately after writing `bootstrap-status`, iterates
`/run/dcc/waiters/*` and signals each one. Concurrent waiters in a durable container each
get their own pipe, so there is no single-consumer problem — the objection that sank the
FIFO idea in r1 applies only to one shared pipe.

**The supervisor must never block signalling a waiter.** A waiter killed between `mkfifo`
and `read` leaves an orphaned FIFO with no reader; a plain `printf > fifo` then blocks
forever *in PID 1*, wedging the container. Opening read-write instead never blocks:

```sh
{ exec 3<>"$f"; printf 'go\n' >&3; exec 3>&-; } 2>/dev/null || true
```

This was verified empirically, not assumed. A blocking write to an orphaned FIFO hung
(`timeout` exit 124); the `<>` form returned 0 immediately, under both `dash` and `bash`.
The full handshake was also exercised for concurrent waiters, a failure status, a
late/steady-state waiter, and an orphaned FIFO alongside a live waiter — the supervisor
completed without hanging in every case.

`bootstrap-status` is written atomically (write `.tmp` in the same tmpfs, then `mv`) so a
waiter can never observe a partial line.

`dcc-ctl wait-ready` then:

- exits `0` if the contents are `0`;
- otherwise prints the failed hook name and the tail of `hook.log` to stderr and exits
  `252` (distinct from the existing `253` "shutting down").

Why not `inotifyd`/`inotifywait`: the FIFO handshake reaches the same event-driven
behavior with zero dependencies, using only `mkfifo` and a shell redirect that POSIX
requires. Since D0 now permits installing packages, this is a preference rather than a
forced choice — but a primitive that needs nothing installed is strictly better than one
needing `inotify-tools` on Debian and the differently-shaped `inotifyd` on BusyBox.

The only remaining poll is the **supervisor's own 200 ms drain loop**, which is unrelated
to readiness — it watches the active-command set. It stays at 200 ms.

### On the host side

The host still polls `docker inspect` until the container is running (`wait_for_running`,
D7) before it can `docker exec` the harness. Docker exposes no readiness notification for
this; `docker events` would be a stream to watch rather than a poll, but it is a heavier
dependency for a bounded 10 s window that only detects total launch failure. Left as-is.

A hidden `--timeout <secs>` on `wait-ready` exists for deterministic tests only; it is not
wired to a user-facing flag. There is deliberately no default wall-clock timeout: a hook
that legitimately takes an hour must not be killed, and every *failure* mode is already
bounded (a failing hook writes a status; a dead supervisor is PID 1, so the container and
every `docker exec` in it die with it).

## D2 — Hook Delivery: Pre-Substituted Scripts In The `rt` Directory

The host writes one executable POSIX `sh` script per hook source into
`.dcc/<profile>.rt/start-hooks/`, named `NN-<sanitized-source>` so lexical order is
execution order:

```
start-hooks/00-feature-ghcr.io_devcontainers_features_node_1
start-hooks/01-feature-._local-feat
start-hooks/02-devcontainer
```

Feature hooks first in installation order, then the devcontainer hook — matching the
current `run_runtime_hooks` ordering exactly. The supervisor is told where they are via
`--start-hooks <dir>`; **absent means run nothing**.

This reuses the mechanism already proven by `build_prep_hook_assets` in
`src/features/mod.rs`, and rides the existing read-only `rt` bind mount, so hook scripts
inherit the supervisor's tamper-resistance.

`RtDir::materialize` clears and recreates `start-hooks/` on every call. Combined with the
explicit `--start-hooks` flag, this doubly guarantees the build-preparation container
(which calls `materialize` but passes no `--start-hooks`) can never execute a stale
runtime hook left by a previous `dcc exec`.

With the fast path gone (D0), hook scripts could alternatively be baked into the image.
They are kept in the `rt` mount anyway: hooks change whenever `devcontainer.json` changes,
and regenerating a directory is far cheaper than an image rebuild per hook edit.

> **Update (T-0028):** the conclusion (hooks stay on the `rt` mount) still holds, but the
> reason above is not the operative one. T-0028 accepted that a `devcontainer.json` edit
> may require a rebuild, so hook volatility is no longer a justification. Hooks stay
> host-delivered because `postStartCommand` may contain `${localEnv:VAR}`, which reads the
> invoking user's host environment and therefore cannot be resolved at image build time.
> See `readme/decisions/0004-embed-supervisor-in-image.md` Q3.

### Object-form (parallel) hooks

`LifecycleCommand::Parallel` runs entries concurrently and reports the first failure after
all finish. The generated script preserves this: each argv is backgrounded, then `wait`ed
on, with the first non-zero status remembered and returned. Single-command hooks are
emitted as a plain invocation.

Argv elements are emitted with POSIX single-quote escaping (`'` → `'\''`) so no
user-supplied string can break out of the generated script.

## D3 — `${containerEnv:…}` Substitution Stays Host-Side

**This resolves the brief's escalation trigger without escalating.**

Because the host generates hook scripts (D2) *before* `docker run`, every hook string is
already fully substituted by the existing path: `apply_substitution` for host/`localEnv`
values, then `resolve_container_env` against env probed from the image via
`inspect_image_env` / `probe_user_env`.

Nothing about substitution moves into the container. Semantics are bit-identical to
today, including the undefined-variable error, so there is no config-compatibility
decision to escalate.

## D4 — `--skip-lifecycle` Needs No Wire Protocol

Under `--skip-lifecycle` the host writes no scripts into `start-hooks/`. The supervisor
finds an empty directory, runs nothing, and writes `0` immediately. The existing
`skipped_hook_warnings` output still prints to host stderr from the host process,
unchanged.

## D5 — Hook Failure Reporting

The supervisor runs each hook with output tee'd to `/run/dcc/hook.log` and to PID 1's own
stdout/stderr (so `docker logs` works too). On failure it writes
`<exit-code> <hook-name>` to `bootstrap-status` and **does not exit** — it stays alive so
the harness can observe and report the failure, rather than the container vanishing and
leaving the host with an opaque "container not running" error.

The failure reaches the user's terminal because `wait-ready` runs inside the harness's
`docker exec`, whose stderr the host inherits. The host maps harness exit `252` to a clear
message naming the failed hook.

A one-shot container whose startup failed still drains and exits once the harness
deregisters, so a failing hook leaks nothing.

## D6 — `postAttachCommand` Stays Host-Side

`postAttachCommand` is per-invocation, not per-container: it runs on every `dcc attach`,
including attaches to an already-running container whose startup hooks ran long ago.
Moving it into the supervisor's startup phase would change its semantics.

It stays where it is, with one addition: on the cold-start path the host runs
`dcc-ctl wait-ready` **before** the attach hooks, so `postStartCommand` is guaranteed to
complete before `postAttachCommand` begins. This preserves the ordering asserted by the
existing `lifecycle_hooks_run_in_expected_phases` smoke test.

## D7 — `wait_for_running` Is Kept

It remains the only thing distinguishing "the container failed to start at all" (bad
image, invalid mount, bad `runArgs`) from "the container is up". Without it such a failure
surfaces as a confusing `docker exec` error against a nonexistent container. It is a
100 ms poll against a 10 s timeout and is not on the critical path for hook duration.

## D8 — Startup Race And Orphan Containment: No Sentinel Files

The startup race is: the supervisor sees an empty active set before the harness registers,
drains, and exits before the user's command lands.

Earlier drafts closed this with sentinel files (T-0024's `primed`, then an
`expect`/`launched` token pair). **Both are unnecessary.** The supervisor is a single
long-running process, so it can hold the one bit it needs — "has a command ever
registered?" — in a *shell variable*:

```sh
arrived=0
while true; do
    n=$(active_count)
    [ "$n" -gt 0 ] && arrived=1
    ...
done
```

This is strictly better than a file: no ordering race between writer and clearer, no
tmpfs path for container-side code to tamper with, and nothing to clean up.

### Drain rules

Let `n` be the active-command count, and `since_bootstrap` the seconds since
`bootstrap-status` was written.

| Condition | Action |
| --- | --- |
| `stopping` exists and `n == 0` | Exit (drain) |
| `oneshot`, `arrived == 1`, `n == 0` | Exit (normal one-shot teardown) |
| `oneshot`, `arrived == 0`, `since_bootstrap >= 10` | **Exit (orphan reaper)** |
| `durable`, no `stopping` | Stay alive |

No other clock exists in the supervisor. `STARTUP_GRACE_SECS`, `PRIMED`, the `started`
variable and the whole grace branch are deleted.

### Why a 10s reaper cannot reintroduce the T-0025 defect

The old 60s grace was fragile because it had to cover *hook execution* — hooks ran on the
host, between launch and the command `docker exec`, so an `npm ci` in `postStartCommand`
could outlast any fixed window.

That is structurally gone. Hooks now run inside the supervisor, and the harness registers
within milliseconds of `docker run`, entirely independent of hook duration. The reaper
clock also starts **after** `bootstrap-status` is written, not at boot, so a two-hour
`postStartCommand` never brings the window closer. The 10 seconds covers only the
host-side gap between `docker run` returning and `docker exec` registering.

The reaper is one-shot-only. Durable containers never reap, so `dcc start` (which
deliberately runs no command) and the build-preparation container are unaffected.

### `--expect-command` is accepted but redundant

Every one-shot launch is by definition a launch that expects a command (`dcc exec`/`dcc
run`); `dcc start` and `--keep` are born durable. The flag is therefore implied by
`--mode oneshot`. It is still accepted, as the brief's acceptance criteria require, and it
makes the launch line self-documenting — but drain logic keys off mode, `arrived`, and the
reaper, not off a token.

## Sequence

Cold start, `dcc exec` (one-shot):

```
host: materialize rt dir + start-hooks/     (hooks fully substituted here)
host: docker run -d --entrypoint dcc-supervisor IMAGE \
        --mode oneshot --expect-command --start-hooks <rt>/start-hooks
host: wait_for_running                       (container exists at all?)
      ├─ sup: write mode, mkdir active + waiters
host: docker exec dcc-exec <argv>            (host does NOT wait for hooks)
      ├─ sup: run start hooks ─────────┐     ├─ harness: register record
      │                                │     ├─ harness: mkfifo waiters/<id>
      │                                │     ├─ harness: status set? no → block on FIFO
      ├─ sup: write bootstrap-status ───┤
      └─ sup: signal every waiters/* ───┴───────────────────────────────────► wakes
         sup: arrived=1 (sees record)         ├─ harness: exec user command
                                              └─ harness: deregister, exit N
      sup: n==0, arrived==1, oneshot → exit; --rm removes
```

If the host is killed before registering, `arrived` stays `0`, and 10 s after
`bootstrap-status` the reaper exits the container rather than leaving it idle.

## Changes By File

| File | Change |
| --- | --- |
| `src/supervisor.rs` | `--mode`/`--expect-command`/`--start-hooks` parsing; hook runner; `bootstrap-status` + `hook.log` + `waiters/`; FIFO signalling via `<>` open; `wait-ready` verb (FIFO block, `--timeout` for tests); `arrived` + 10 s reaper; delete `STARTUP_GRACE_SECS`, `PRIMED`, `MODE_ENV`; hook-script generation + `sh_quote` |
| `src/exec.rs` | Emit start-hook scripts; replace `-e DCC_MODE` with entrypoint args; drop `run_runtime_hooks(Startup)`; host `wait-ready` for `start` and the attach path; map exit `252`; keep `Attach` hooks host-side |
| `src/build.rs` | `--mode durable` as an entrypoint arg instead of `-e DCC_MODE` |
| `src/stop.rs` | Unchanged |
| `README.md`, `readme/project/architecture.md` | Revised startup sequence |

D0 (fast-path removal) touches `build.rs`, `version.rs`, `run.rs`, `stop.rs`, and
`uid.rs`, and is scoped to **T-0027**, not to the table above.

## Risks Accepted

- A hook that hangs forever hangs the command. Unchanged from today; Ctrl-C on the host
  kills the `docker exec` as before.
- Profiles that previously took the fast path now pay one extra cached `docker build`
  stage. Accepted per D0, and realized by T-0027.
- `hook.log` lives in the container-private tmpfs and is readable by container-side code.
  It contains hook output, which that code could produce anyway. Consistent with the
  T-0024 posture.
- A waiter that dies between `mkfifo` and `read` leaves an orphaned FIFO in the tmpfs. The
  supervisor's `<>` open makes signalling it a no-op, and the tmpfs dies with the
  container.

## Material Amendments

| Revision | Date | Source | Change | Reason |
| --- | --- | --- | --- | --- |
| r1 | 2026-08-13 | Design pass | Initial design | — |
| r2 | 2026-08-13 | Review feedback | Single `bootstrap-status` file replacing `ready`/`failed`; 10 s one-shot orphan reaper; sentinel files replaced by a supervisor-local `arrived` variable; inotify evaluated and rejected with a 50 ms readiness poll instead | Simpler status interaction, bounded orphan containment, less state |
| r3 | 2026-08-13 | Review feedback | D0: remove the image fast path, split out as catalog task T-0027. D1: in-container readiness wait becomes event-driven via a per-waiter FIFO (empirically validated, incl. the orphaned-FIFO wedge and its `<>` fix) instead of a 50 ms poll | Fast path taxed every core feature; readiness wait should be event-driven, and a FIFO achieves it with no package dependency |
