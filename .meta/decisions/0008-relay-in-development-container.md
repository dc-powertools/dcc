# 0008: Run The Relay Inside The Development Container

Status: Accepted placement; detailed design pending

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

## Consequences And Remaining Design

- Keep relay infrastructure outside the user-command active set so one-shot and
  graceful container teardown remain possible.
- Enable forwarding for runtime containers only; the shared supervisor also runs
  in build-preparation containers.
- Define proxy implementation/provisioning, internal port allocation, readiness,
  failure handling, and shutdown behavior before implementation.
- Resolve container-network exposure, supported network modes, remote Docker
  semantics, and configuration changes on existing containers explicitly.
- Follow decision 0004 for any host/supervisor protocol change and version boundary.

This records the placement decision, not a completed detailed design or an
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
owns that policy and the proposed fingerprint/snapshot mechanism. It applies to
forwarding as well as other configuration; no automatic reconfiguration or restart.
