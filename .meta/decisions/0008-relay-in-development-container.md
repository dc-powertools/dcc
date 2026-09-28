# 0008: Run The Relay Inside The Development Container

Status: Accepted placement and behavior; detailed implementation design pending

Date: 2026-09-28

Authority: user selection of option 1 following T-0095: "Option 1 is the only
viable option. We will not use a separate container."

## Decision

Place the relay inside the development container as a child service managed by
the existing PID 1 supervisor. Its lifetime follows the container, independently
of foreground user sessions. Use Docker publication for the host-facing leg and
have the proxy connect to the application's container-loopback address.

A separate relay container is excluded. The host-helper and host-service
alternatives examined in T-0095 are not selected.

## Implementation Design Boundaries

- Keep relay infrastructure outside the user-command active set so one-shot and
  graceful container teardown remain possible.
- Enable forwarding for runtime containers only; the shared supervisor also runs
  in build-preparation containers.
- Specify the utility integration, readiness handshake, child cleanup, retry limits,
  internal port range/default and configuration field, and IPv6 creation fallback
  within the accepted policies below.
- Explain the isolation/complexity tradeoff and map the accepted network support
  boundaries to concrete checks and diagnostics.
- Follow decision 0004 for any host/supervisor protocol change and version boundary.

This records the placement and behavioral decisions, not a completed detailed design or an
instruction to implement. The user's no-reproduction constraint remains in force.
The candidate examination is in `../tasks/0095-relay-placement.md`.

## Access Policy Clarification — 2026-09-28

The user prefers preserving the existing isolation of loopback-only applications,
but accepts access through the proxy from peer containers on the Docker network
if preserving isolation introduces too much complexity. Favor the simpler design
in that case. Host publication remains limited to localhost.

This is a conditional design tradeoff, not a requirement to add network isolation
machinery. The detailed design should explain the chosen exposure and complexity;
no isolation implementation or product-code change is selected by this clarification.

## Internal Proxy Port Allocation — 2026-09-28

The user accepted deterministic allocation from a documented, configurable internal
proxy-port range, skipping application target ports declared in `forwardPorts`.
Only the allocated ports are reserved for the container's lifetime. Projects can
override the range to avoid their other application ports. Bind failures must be
reported clearly; elaborate automatic remapping is not required.

The example proxy port 20000 was illustrative. The default range and configuration
field remain implementation-design details, not selected values. Dynamic allocation
does not remove the need to reserve ports or coordinate Docker's publication mapping.

## Startup Readiness — 2026-09-28

The user accepted startup success only after Docker establishes the configured
host-port mappings, every internal proxy listener is bound and ready, and existing
startup-hook requirements complete. Application listeners need not exist yet;
developers may start applications after attaching.

A proxy bind failure fails the startup command with the affected port and reason.
Retain the container for diagnosis when possible, consistent with existing
startup-hook failure behavior. If the proxy is ready but its application is not
listening, only the incoming connection fails; application unavailability alone
does not make the container unhealthy.

## Relay Failure After Startup — 2026-09-28

The user accepted bounded supervisor restart attempts with a short delay after a
relay failure. If recovery fails, keep the container and user commands running.
Subsequent `dcc start`, `attach`, and `exec` report degraded forwarding while
retaining shell access for diagnosis. Existing connections may be lost on a crash.

Application connection failures do not trigger relay restarts. Restart attempts
stop when container shutdown begins, and relay infrastructure cannot keep a
one-shot container alive. Exact retry limits and delays remain design details.

## Configuration Drift — 2026-09-28

The user broadened warn-and-defer behavior to all easily detectable configuration
changes while a container is running. [Decision 0009](0009-warn-and-defer-running-config-changes.md)
owns that policy and the accepted fingerprint/snapshot approach. It applies to
forwarding as well as other configuration; no automatic reconfiguration or restart.

## Proxy Provisioning — 2026-09-28

The user accepted the recommended packaged-utility approach, with `socat` as the
initial candidate. Reuse a suitable installed version or provision it during image
build. Fail the build clearly if the required utility cannot be provided. A dedicated
dcc relay executable is not the selected starting approach.

The chosen utility must still satisfy the accepted readiness, recovery, concurrent
connection, half-close, and child-process cleanup requirements. Exact package/version
support and supervisor integration remain design and verification work; no runtime
verification has been performed.

## Supported Environments And Network Modes — 2026-09-28

The user accepted forwarding on local Docker Engine and Docker Desktop using
normal bridge networking and localhost publication. If forwarding is configured,
reject `--network=none` and `--network=host` as incompatible before creating a new
container. These restrictions do not apply when forwarding is absent.

For a remote Docker daemon, publish on the remote Docker host's localhost and
explicitly report that location. Do not add a tunnel back to the CLI machine.
Configuration changes targeting an existing running container continue to follow
decision 0009's warn-and-defer policy rather than applying new network settings.

Concrete platform verification remains pending.

## IPv4 And IPv6 — 2026-09-28

The user approved required IPv4 publication on host `127.0.0.1` and best-effort
IPv6 publication on `::1`. If IPv6 publication is unavailable, warn and continue
with IPv4. Never fall back to publication on all host interfaces.

The proxy connects to application `127.0.0.1` inside the container, preserving
the current destination behavior. Applications listening exclusively on `::1`
remain outside this change. Docker-managed publication requires a deliberate
container-creation fallback for unavailable IPv6; its mechanism and verification
remain implementation-design work.

## Upgrade And Migration — 2026-09-28

The user accepted an explicit migration for the new supervisor capabilities:

- Use a minor-version protocol boundary, currently 0.2.0 from the 0.1.9 baseline.
- Require explicit image rebuild and container recreation to adopt the new behavior.
- Refuse incompatible runtime operations with clear migration instructions.
- Keep `dcc stop` available for older containers.
- Never replace or modify a running container automatically.
- Validate against the actual running container's image, not merely the current
  image tag, which may have been rebuilt independently.

Ordinary configuration drift continues to warn and defer under decision 0009;
an incompatible supervisor protocol is a separate condition. This accepts the
migration policy, not an instruction to bump the version or publish a release now.
