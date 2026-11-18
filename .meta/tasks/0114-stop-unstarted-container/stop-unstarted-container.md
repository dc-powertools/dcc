# T-0114: Stop created-but-unstarted profile containers

- Authority: user task-recording request 2026-11-17; revision r1.
- Scope: record future implementation and verification; implementation has not started.
- Related context: [T-0113](../../archive/tasks/0113-fast-command-teardown-race/fast-command-teardown-race.md), CLI-crash/reset research. No dependency on fixing the teardown races.

## Problem and outcome

A CLI crash between Docker container creation and startup can leave a profile runtime in created state. No supervisor is running to clean it up; the name can obstruct a subsequent launch. Current dcc stop, stop --now, and stop --kill discover only running containers and therefore report an idempotent success while leaving this obstruction intact.

Have all stop variants also discover and remove the selected profile's created-but-unstarted runtime container. There is no process to signal in this state, so removal is the shared behavior for all variants. Existing running-container behavior and absent-container idempotency remain in force.

## Implementation constraints

- Identify the exact profile/workspace runtime using its managed identity/role and validated container metadata; use the immutable container ID for mutations. Do not remove unrelated or build-preparation containers.
- Establish created/never-started state rather than interpreting any non-running container as abandoned.
- Handle concurrent startup or disappearance safely. If removal finds the container has started, re-inspect the same instance and apply the requested variant's running-container behavior; do not silently force-remove based on an outdated created-state observation or affect a replacement instance.
- Account for a concurrent launching CLI: an explicit stop may cancel that exact attempt, but the launcher must fail clearly rather than accidentally act on a replacement.
- Preserve image, declared persistent state, and other profiles. No deletion of .dcc is required.
- Update dry-run/debug output and relevant command documentation for the added removal behavior.

## Acceptance and verification

- Docker-boundary and live coverage for a created-but-never-started managed runtime under each stop variant; verify removal and successful subsequent launch.
- Verify no-op behavior when absent, existing behavior when running, profile isolation, and refusal to mutate unrelated containers.
- Add deterministic coverage for created-to-running and disappearance races, using exact instance identity.
- Run applicable repository checks and Docker regressions; record any compatibility impact.

Relevant source: src/stop.rs running-only discovery; src/docker.rs container inspection/removal and identity lookup; src/runtime.rs creation-attempt identity and never_started handling.
