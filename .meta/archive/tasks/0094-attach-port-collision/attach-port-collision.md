# T-0094: Attach port collision diagnosis

- Authority: user diagnosis request, 2026-09-28; r2 adds an explicit instruction
  not to attempt reproduction because this environment cannot launch containers.
- Scope: source-based diagnosis only; no product-code changes.
- Status: Done.

## Findings

`src/exec.rs::execute_foreground` correctly reuses a running profile container,
recording `started = false`, but unconditionally calls `forward::forward_ports`
after both the new-container and reuse branches. This applies to `attach`, `exec`,
and named `run` commands (which delegate to `exec`).

`src/forward.rs::bind_forwarding_listeners` requires a fresh IPv4 TCP listener on
`127.0.0.1:<port>` for every configured port. There is no cross-process ownership
registry, sharing, or recognition of an existing relay for the same container.
Each foreground CLI process retains its own listeners until its command completes
and `Forwarding::shutdown` runs. Consequently, a second foreground invocation
collides with a still-active first invocation that forwards the same host port.

The bind error propagates through `?` before attach hooks or the shell execute.
This is a fatal error, not just a warning. The reported container name is error
context; the failed bind is on the host, not inside the container.

A running container alone is insufficient to cause the failure. `dcc start`
does not create relays, and a completed foreground invocation shuts its relays
down even if the container remains durable. A process must still occupy the host
address (or an overlapping wildcard address). Another active dcc session is the
expected explanation for the concurrent-session case; an unrelated host service,
IDE relay, or published Docker port can produce the same error. The supplied error
does not identify the actual listener owner on the user's machine.

## Correction direction

Container reuse and forwarding ownership need consistent behavior for concurrent
sessions. A robust shared-forwarding design would verify ownership by container
and endpoint and keep the relay alive while dependent sessions need it. Merely
skipping forwarding whenever a container is running breaks `start` followed by
`attach`; ignoring all address-in-use errors can silently accept an unrelated
service. An explicit option to disable forwarding for an additional session is
a smaller possible workaround, but does not currently exist.

## Evidence and limits

- Read the foreground reuse/setup/teardown paths, listener acquisition and cleanup,
  `start`, the `run` delegation, CLI flags, and documented forwarding semantics.
- Existing test `later_bind_collision_releases_every_listener_without_spawning_tasks`
  establishes the intended fatal-collision/cleanup contract in source; it was not run.
- No reproduction or container launch was attempted. A temporary fake-Docker
  reproduction script was prepared but never run, then removed after the user
  clarified the constraint.
- `cargo build --locked` was started to prepare that investigation, then cancelled;
  it is not a passing verification result.
- Documentation consistency and `git diff --check` passed. Runtime validation is
  outside the revised source-only diagnosis scope.

The actual host listener can be identified separately on the affected machine
with `lsof -nP -iTCP:4173 -sTCP:LISTEN`; no such inspection was possible here.
