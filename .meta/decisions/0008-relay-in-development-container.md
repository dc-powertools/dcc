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
