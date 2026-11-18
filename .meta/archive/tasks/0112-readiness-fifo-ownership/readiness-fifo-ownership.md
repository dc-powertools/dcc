# T-0112: Fix readiness FIFO ownership

- Authority: user implementation and commit request 2026-11-17; revision r2.
- Scope: implement and verify the readiness user fix, then create a local commit.
- Evidence: [T-0111 reproduction](../0111-reproduce-cold-one-shot-startup-status-137-and/quality-startup-reproduction.md).

## Completion

Implemented and verified on 2026-11-17. Readiness controls inherit the supervisor's configured user; other controls retain root. Deterministic ownership/notification
regressions fail before and pass after the fix. All 34 Docker smoke tests, the
regular suite, formatting, Clippy, and build passed. No baked supervisor/protocol
change or compatibility bump required. See [verification](quality-readiness-fifo-ownership.md).

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
