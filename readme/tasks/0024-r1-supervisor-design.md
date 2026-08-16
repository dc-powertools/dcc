# T-0024-R1 Design: In-Container Supervisor

Deliverable of sub-task T-0024-R1. Parent brief:
`readme/tasks/0024-supervisor-lifecycle-ownership-brief.md`.

## Decisions

### D1 — Supervisor is a POSIX shell script, delivered by read-only bind mount

> **Superseded by `readme/decisions/0004-embed-supervisor-in-image.md` (T-0028).** The
> supervisor scripts are now baked into the image; only `postStartCommand` hook scripts
> remain on the `rt` bind mount. The deciding constraint below (the image fast path) was
> removed by T-0027, and the tamper-resistance rationale was explicitly discarded as a
> non-goal. The POSIX `sh` language choice recorded at the end of this section still
> stands. Retained for history; do not cite as current rationale.

**Blocker found in research:** `uses_fast_path` (`src/build.rs`) pulls the user's image and
`docker tag`s it without running `build_dcc_stage`, so `generated_assets()` never reach the
image. A supervisor baked into the image would be **absent from every fast-path container**.

**Decision:** generate the supervisor scripts from a single Rust source of truth
(`src/supervisor.rs`), write them to a host directory `.dcc/<profile>.rt/`, and bind-mount
that directory **read-only** into the container at `/usr/local/share/dcc/rt`.

Rationale (as recorded at the time; first two points no longer hold):

- Works identically on the fast path and the full build path; no image rebuild required.
- Read-only: container-side code cannot tamper with the supervisor itself. The T-0023
  exposure was *writable* host-backed control state; read-only executable code is not that.
- `/usr/local/share/dcc` is already a T-0021 Tier-1 reserved subtree, so users cannot
  target it with `customizations.dcc.state`. The mount point inherits that protection for
  free.
- `.dcc/<profile>.rt/` is a sibling of the cache root, matching the T-0022
  `.dcc/<profile>.seed.json` convention, so it is **not** inside the `/cache` mount.

POSIX `sh` (not a static binary): the current keep-alive `tail -f /dev/null` was chosen for
glibc/BusyBox/Alpine portability and the supervisor must meet the same bar. A shell script
needs no per-architecture distribution story.

### D2 — Lifecycle state lives in a container-private tmpfs

`--tmpfs /run/dcc:mode=1777` gives the supervisor a writable, container-private state
directory that dies with the container. Layout:

| Path | Meaning |
| --- | --- |
| `/run/dcc/active/<id>` | One file per registered command |
| `/run/dcc/mode` | `oneshot` or `durable` |
| `/run/dcc/stopping` | Present once a graceful stop has been requested |
| `/run/dcc/primed` | Present once the first command has ever registered |

This is *not* host-backed. Container-side code can reach it, which is accepted: per the
task posture, failures that cannot escape the container are tolerated, and remediation is
`dcc stop --kill`.

### D3 — Control protocol is `docker exec` of a control script, not a socket

No socket, no daemon protocol. The host CLI drives the supervisor by `docker exec`-ing
`/usr/local/share/dcc/rt/dcc-ctl` with a verb:

| Verb | Effect |
| --- | --- |
| `mode durable` | Promote to durable (`--keep` on an already-running container) |
| `stop` | Create `stopping`; supervisor drains then exits |
| `stop-now` | Create `stopping`, `TERM` all registered commands, supervisor runs shutdown hooks and exits |

Commands are registered by a wrapper rather than a separate verb, so registration and the
command's lifetime cannot desynchronize:

```
docker exec [-i] [-t] <container> /usr/local/share/dcc/rt/dcc-exec <id> <argv...>
```

`dcc-exec` creates `/run/dcc/active/<id>`, runs `argv` as a child, and removes the record in
an `EXIT` trap — so the record disappears on normal exit, error, or signal death. It exits
with the child's status, preserving exit-code propagation to the host.

### D4 — Initial mode is set at `docker run` time

`-e DCC_MODE=oneshot|durable` is passed on `docker run`. The supervisor reads it at startup
and writes `/run/dcc/mode`. This removes the mode race entirely: a one-shot container is
born one-shot, a `dcc start` container is born durable. Only `--keep` against an
*already-running* container needs the `dcc-ctl mode durable` call.

### D5 — Drain rules (the correctness-critical part)

The supervisor polls every 200 ms and exits only when **all** hold:

1. `/run/dcc/active` is empty, and
2. `/run/dcc/primed` exists (at least one command has registered) **or** the startup grace
   period has elapsed, and
3. mode is `oneshot` **or** `/run/dcc/stopping` exists.

Rule 2 closes the startup race where the supervisor would see an empty set and exit before
the first `docker exec` lands. The startup grace (60 s) prevents an orphaned container when
the first `docker exec` never arrives.

A durable container never drain-exits until a `stop` is requested — rule 3.

Registration is refused once `stopping` exists, which is what makes graceful stop mean
"accept no new work, finish what is running."

### D6 — Stop variants

| Command | Mechanism |
| --- | --- |
| `dcc stop` | `dcc-ctl stop`, then wait for the container to disappear (drain) |
| `dcc stop --now` | `dcc-ctl stop-now`: `TERM` registered commands, run shutdown hooks, exit |
| `dcc stop --kill` | `docker kill`, no container cooperation |

If the container is not running, all three are idempotent successes, matching today's
behavior. If `dcc-ctl` cannot be reached (wedged container), `dcc stop` reports the failure
and points at `--kill`.

### D7 — Shutdown hooks

A new `RuntimeHookPhase::Shutdown` is **not** added in this task. `--now` runs any
build-prep-style shutdown assets present under the rt directory; there is currently no
`postStopCommand` in the devcontainer schema that `dcc` honors. The supervisor calls an
optional `/usr/local/share/dcc/rt/dcc-shutdown` if present, leaving a seam for a future
hook phase without inventing config surface now.

## Consequences for existing code

- `src/runtime.rs` is deleted in full (`RuntimeState`, `ContainerMode`, `ActiveCommand`,
  `RuntimeLock`) along with its five unit tests.
- `src/exec.rs` loses `finish_active_command` and all lock/record/mode bookkeeping; it
  gains `-e DCC_MODE`, the rt bind mount, the `/run/dcc` tmpfs, and `--entrypoint
  /usr/local/share/dcc/rt/dcc-supervisor`.
- `src/build.rs` build-prep container gets the same entrypoint, tmpfs, and rt mount.
- `src/stop.rs` gains the three variants and drops `RuntimeState::new(...).clear()`.
- `generated_assets()` drops the dead `dcc-controller` and `dcc-command` placeholders
  (never referenced at runtime; superseded by the rt directory) and keeps hook assets.

## Accepted failure modes

- A `dcc run` arriving after `stopping` is set is refused by `dcc-exec` and fails with a
  clear, retryable message.
- Two simultaneous launches race on the container name; one fails.
- A wedged supervisor is remediated by `dcc stop --kill`.
