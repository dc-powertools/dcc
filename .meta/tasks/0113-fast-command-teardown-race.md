# T-0113: Diagnose and fix fast-command teardown race

- Authority: user request 2026-11-17 to record a task; revision r1.
- Scope: future diagnosis, implementation and verification; this intake does not begin implementation.
- Evidence: [T-0111 reproduction](../quality/0111-startup-reproduction.md).

## Problem and outcome

In an isolated local Codex pilot fixture, three cold `dcc exec codex --version` runs returned 0, 137, 0. On the failing attempt, wait-ready and snapshot execs had succeeded; the command exec then died with 137 while the container exited normally with 0. This is distinct from the startup FIFO failure in T-0112. The exact teardown interleaving remains to be established.

Investigate whether removing the active-command record before the Docker exec wrapper has fully exited allows the one-shot supervisor to terminate the container before Docker captures command completion. Also inspect active-count/arrived-marker ordering and concurrent registration versus drain. These are hypotheses, not a confirmed root cause. Ensure short commands reliably deliver their actual exit status and output before automatic container teardown.

## Acceptance and verification

- Capture a reproducible event sequence and establish the failing interleaving; do not rely only on probabilistic repeated runs.
- Add a meaningful deterministic regression around command completion versus supervisor drain, using explicit coordination where needed.
- Verify successful and nonzero short commands preserve exact status and output, including commands completing between supervisor polling intervals.
- Verify concurrent commands are not terminated prematurely; preserve one-shot cleanup, durable/--keep behavior, stop behavior, and command registration safety.
- Avoid relying on a fixed arbitrary delay as proof that command completion has been observed.
- Run applicable repository checks and Docker regressions; assess host/supervisor compatibility and version requirements.

Relevant source: src/supervisor.rs exec_script registration, EXIT cleanup and supervisor active_count/drain loop; src/exec.rs execute_in_runtime; Docker foreground exec handling.

Original fixture: /tmp/dcc-repro-0111-8_m65wh4. Captured events: /tmp/dcc-repro-0111-codex-events.jsonl. Temporary artifacts may expire; durable observations are in T-0111.
