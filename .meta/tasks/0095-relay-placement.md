# T-0095: Candidate relay placements

- Authority: user architecture examination request and selection of the existing
  container supervisor and subsequent isolation/allocation/readiness decisions,
  2026-09-28 / r4.
- Status: Done; placement selected, no implementation performed.
- Constraint: source/documentation inspection only; no reproduction, container
  commands, builds, or runtime tests.

## Accepted placement (r2)

The user selected option 1: a relay inside the development container, managed by
its existing PID 1 supervisor. Separate containers are explicitly excluded.
[Decision 0008](../decisions/0008-relay-in-development-container.md) owns this
selection and the remaining design boundaries. The candidate comparison below is
historical examination, not a set of still-open placement choices.

The r3 discussion accepted a preference for simple isolation with peer-container
access permissible when isolation is too complex, and deterministic proxy-port
allocation from a configurable range. Decision 0008 owns these accepted policies;
the remaining design concerns are still under discussion.

The r4 discussion accepted readiness requiring Docker mappings, all proxy listeners,
and existing startup hooks, without requiring application listeners. Decision 0008
also records startup-failure diagnostics and retaining the container where possible.

## Existing boundaries

- `src/exec.rs::RuntimePlan::prepare` assembles one detached, auto-removed runtime
  container, using `containerUser`, workspace/cache/state mounts, read-only startup
  hooks, and private `/run/dcc` tmpfs. `start_container` is shared by `start` and
  foreground commands. Container lookup uses a stable project/profile label and name.
- `src/supervisor.rs::supervisor_script` is a baked POSIX shell PID 1. It owns
  bootstrap/readiness, durable versus one-shot mode, user-command draining, orphan
  timeout, and shutdown. It is not currently a general child-service manager.
- `dcc-exec` registers user commands in `/run/dcc/active`; infrastructure must not
  register there, or a permanent relay would prevent one-shot and graceful teardown.
- `src/forward.rs` binds host IPv4/optional IPv6 loopback in the foreground CLI.
  Each connection launches `docker exec -i ... dcc-connect 127.0.0.1 PORT`.
  `dcc-connect` is already inside the container but is a one-connection stdio/TCP
  adapter, not a persistent listener. It does not register as a user command.
- `src/features/mod.rs::generated_assets` and `src/features/context.rs` bake the
  supervisor/connector and provision a compatible netcat. No persistent proxy
  server, host daemon, or sidecar currently exists.
- `src/stop.rs` signals the supervisor or falls back to Docker stop/kill; a relay
  must also handle one-shot exit and termination outside this command.

Logical lifecycle ownership does not require every forwarding component to live
inside the container. A normal container cannot bind the developer host's loopback
interface directly. Moving the relay into it therefore also requires a host-to-
container transport change or a retained host listener.

## Candidates

| Placement | Transport and ownership | Fit and principal cost |
| --- | --- | --- |
| One detached host helper per runtime container | Existing host listeners and `docker exec` connectors move into a long-lived helper; helper observes the actual container's lifetime. | Least transport change; preserves CLI-host loopback and container-loopback access, including `--network=none`. Adds background-process launch, locking, readiness, crash recovery, and cleanup ownership. |
| Child service of the existing in-container supervisor, plus Docker publishing | Docker publishes host loopback PORT to a separate container proxy port; proxy connects to container `127.0.0.1:PORT`. PID 1 starts and monitors the proxy; Docker owns the host mapping. | Strongest fit with current centralized lifecycle ownership; no new dcc host daemon. Requires a real proxy implementation, provisioning, runtime mapping/configuration, readiness/failure handling, and network exposure analysis. |
| Companion container sharing the runtime network namespace | Sidecar uses `--network container:<runtime-id>` so its proxy can reach application loopback. Host publishing belongs on the namespace-owning runtime container. | Isolates proxy dependencies from arbitrary development images. Adds a second image/container, readiness ordering, explicit lifetime coordination and recreation handling; sharing networking does not share lifecycle. |
| One host service managing all dcc relays | Service owns all host listeners and reconciles running containers. CLI requests forwarding over local IPC. | Centralizes ownership and recovery, but introduces a new service installation/control/versioning surface and a larger failure domain. More machinery than a per-container helper. |
| Docker publishing directly to the application | Docker owns the mapping; no dcc relay. | Smallest implementation only if application binding/source-address requirements change. Does not preserve the current guarantee of reaching loopback-only apps. |

## Supervisor-hosted proxy details

Illustrative path (54173 is an example, not a proposed reserved port):

```text
host 127.0.0.1:4173
  -> Docker publication to container network-address:54173
  -> supervisor-managed TCP proxy
  -> container 127.0.0.1:4173
```

This preserves a loopback peer at the application even though the first leg uses
Docker publishing. The previous suggestion that publishing necessarily changes the
application's loopback behavior applied to direct publishing, not this two-stage
design.

The proxy should be a dedicated baked executable/service, launched by PID 1, not
a `postStartCommand` or a permanent `dcc-exec` invocation. User lifecycle hooks can
be skipped and do not provide child-service health management. The shared supervisor
is also used during build preparation, so forwarding must be explicitly enabled only
for runtime containers. No need to replace the existing POSIX supervisor merely to
add a child service.

Publishing a different internal proxy port avoids occupying the application's
port when the app binds `0.0.0.0`. Allocation must still handle conflicts with
other application ports; no fixed reserved range is assumed safe. Mappings must
be planned at container creation and configuration drift needs an explicit policy.

Docker can bind publication to host loopback, including through Docker Desktop.
However, a proxy listening on the container's network interface is also reachable
by allowed peers on that network. Host-loopback publication alone does not retain
the existing container-side loopback isolation. Network isolation/filtering or
another protected transport needs design review. `--network=none` would also need
an explicit unsupported-mode policy or fallback. With a remote Docker daemon,
publication refers to the daemon host, whereas today's listener is on the CLI host.

New supervisor launch/configuration protocol must follow decision 0004's
minor-version compatibility boundary; this is not assumed to be a patch-only fix.

## Host-helper details

Use a background mode of the existing binary to reuse `Forwarding`, rather than
introducing a second installed program. Runtime startup/reuse should idempotently
ensure it exists, with an explicit startup result before reporting success. Key
ownership by Docker endpoint plus immutable container instance ID; the existing
project/profile label and reusable name alone are insufficient across recreation.

A `docker wait` observer is a candidate exit signal, with reconciliation for
removal, daemon disconnection, and startup races. Preserve the invoking Docker
endpoint/context for subsequent connections. Host metadata must not depend on
container-private tmpfs or ephemeral foreground task handles. Recovering a dead
helper and cleaning stale ownership require deliberate design; session reference
counts would reintroduce the wrong lifetime.

## Shortlist and limits

The r1 examination shortlisted the supervisor-managed proxy and per-container host
helper. The user's r2 selection supersedes that shortlist: only the proxy managed
by the existing development-container supervisor remains eligible.

Host networking is not a default solution: it changes isolation, conflicts with
profile port independence, is gated by current `runArgs` policy, and has platform/
Docker Desktop requirements. A sidecar on an ordinary separate bridge network
cannot reach the development container's loopback service. Mounting the Docker
socket merely to let a sidecar execute connectors would expand authority and is
unnecessary for the shared-network-namespace candidate.

## Verification and sources

Source trace, candidate counterexamples, Markdown table/link inspection, and
`git diff --check` passed. No runtime verification was attempted or claimed.

- [Docker container networking](https://docs.docker.com/engine/network/#container-networks):
  shared network namespaces and the prohibition on publishing from the joining container.
- [Docker port publishing](https://docs.docker.com/engine/network/port-publishing/):
  host-address selection and container-network accessibility. Older Engine versions
  also have a documented localhost-publication exposure caveat to consider in design.
- [Docker Desktop networking](https://docs.docker.com/desktop/features/networking/):
  Desktop provides the host-facing listener and transport into the Linux VM.
- [Docker wait](https://docs.docker.com/reference/cli/docker/container/wait/):
  candidate observer that blocks until the container stops.
- [Docker host networking](https://docs.docker.com/engine/network/drivers/host/):
  isolation and platform constraints.
