# T-0112: Fix readiness FIFO ownership

- Authority: user request 2026-11-17 to record a task; revision r1.
- Scope: future implementation and verification; this intake does not begin implementation.
- Evidence: [T-0111 reproduction](../quality/0111-startup-reproduction.md).

## Problem and outcome

`docker::control` executes wait-ready and wait-hooks as root. A waiter registered before bootstrap completes creates a root-owned 0644 FIFO; a non-root supervisor cannot write the notification. Cold one-shot startup then fails with status 137 when the orphan reaper exits the container. Durable startup can hang indefinitely. Reproduced twice with Debian, a one-second startup hook, a forwarded port and /bin/true; container exit code was 0, without an OOM event.

Make readiness notification work for root and non-root supervisors without broadening FIFO access unnecessarily. Proposed approach: execute wait-ready and wait-hooks as the container's configured user, matching the supervisor, while retaining root for control operations that require privileged snapshot access. Validate that choice against all callers and UID mappings before implementation; do not assume every control operation should change user.

## Acceptance and verification

- Add a Docker regression that establishes waiter registration before bootstrap completion; a startup that only exercises the already-complete status fast path is insufficient.
- Verify cold named run, stop then cold run, durable start, and wait-hooks reuse with a non-root supervisor; include root-user coverage.
- Preserve successful and failing hook status reporting, private FIFO access, snapshot confidentiality, and existing control semantics.
- Run applicable repository checks and Docker regressions; assess host/supervisor compatibility and version requirements for the final implementation.
- Keep fast-command teardown remediation in T-0113, with no dependency on a speculative teardown diagnosis.

Relevant source: src/docker.rs control; src/supervisor.rs ctl_script and signal_waiters; src/exec.rs acquire_runtime; src/runtime.rs load.
