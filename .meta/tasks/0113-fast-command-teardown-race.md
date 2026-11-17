# T-0113: Diagnose and fix fast-command teardown race

- Authority: user task-recording request, subsequent design/reproduction request, explicit task-update requests, and user preference for the current approach after crash analysis, 2026-11-17; revision r5.
- Scope: investigation includes user-requested post-launch interactive disconnect experiments; product implementation remains pending. Pre-command CLI-disconnection detection remains deferred.
- Evidence: [initial observation](../quality/0111-startup-reproduction.md), [deterministic race confirmation](../quality/0113-teardown-races.md), and [interactive disconnect observations](../quality/0113-interactive-disconnects.md).

## Problem and outcome

In an isolated local Codex pilot fixture, three cold `dcc exec codex --version` runs returned 0, 137, 0. On the failing attempt, startup and snapshot inspection had succeeded; the command exec then died with 137 while the container exited normally with 0. This is distinct from the startup FIFO failure fixed in T-0112.

Both suspected teardown races are now independently confirmed in isolated Docker containers using temporary checkpoints to force existing scheduling windows:

1. **Stale active count:** pause the supervisor after it counts zero active records; let an unmodified command wrapper register, set the arrived marker, and start a long-running command; resume the supervisor. It combines the old zero count with the new arrived marker and shuts down, killing the running command with 137.
2. **Deregistration before wrapper exit:** let the user command print a completion marker and return 0; pause its wrapper's EXIT trap after removing the active record but before exiting. The unmodified supervisor sees no active commands and shuts down, killing the wrapper with 137.

In both cases, Docker reported container exit 0 and OOMKilled=false. These results confirm both race windows, but do not determine which caused the original intermittent Codex failure. No product source was changed for the experiments, and test containers were removed.

The outcome is to preserve command output and its actual exit status before automatic teardown, and prevent concurrent registration from racing a shutdown decision.

## Investigated alternative: CLI acknowledgment (not selected)

Make command reservations supervisor-owned, with an explicit CLI completion acknowledgment:

1. The CLI acquires a reservation before command execution; startup must protect the initial request from orphan cleanup until acquisition is resolved.
2. The supervisor serializes acquisition, release, and transition to draining. New acquisitions after draining begins are rejected so the caller can retry safely on a fresh container.
3. The CLI runs the Docker exec and receives its output and exit status.
4. Only after Docker exec returns does the CLI release the reservation. The wrapper no longer removes the reservation that authorizes teardown.
5. An idle one-shot container may shut down only after all accepted reservations have been released. Preserve durable and explicit-stop semantics.

This separates command execution from permission to tear down and avoids using a filesystem count sampled independently of registration. It also avoids equating wrapper deregistration with Docker observing command completion.

The proposed protocol would also need idempotent acquisition/release, instance-token fencing, and explicit CLI crash/disconnection recovery. Reservations accepted without a surviving caller could otherwise block teardown indefinitely. Reset remains possible by killing and replacing the container, but graceful stop and stop-now would need defined behavior for abandoned reservations. A protocol change would require at least a minor version bump under the existing compatibility contract.

## Crash analysis and user conclusion

After comparing crash consequences, the user favors retaining the current container-owned registration/cleanup approach rather than requiring a CLI completion acknowledgment (2026-11-17). Keep this task open for further research. This selects a design direction; it does not resolve or accept either confirmed race, approve an implementation, or authorize task closure.

The differences that informed this preference:

| Crash point | Current approach | Proposed acknowledgment approach |
| --- | --- | --- |
| Reservation accepted, but CLI dies before receiving acceptance | | Reservation can remain even though execution never begins. |
| CLI dies before launching the command | An unregistered one-shot container normally exits about 10 seconds after bootstrap completes. | If a reservation was acquired, it blocks teardown until explicit release, reclamation, or reset. |
| CLI dies during execution and the wrapper later exits normally | Wrapper cleanup normally allows automatic teardown. | No CLI remains to acknowledge; reservation keeps the container alive after command completion. |
| CLI dies after command completion, before acknowledgment | Normal wrapper cleanup allows automatic teardown. | Unreleased reservation requires explicit release, reclamation, or reset. |
| CLI dies during acknowledgment | | Release may or may not have occurred; retries must be harmless, and the container may already have exited. |

The current approach is not crash-proof: an uncatchably killed wrapper can leave a stale active record, and a CLI can lose output or knowledge of command success. Those are not advantages of the acknowledgment design by themselves. Retaining a reservation protects lifetime but does not itself preserve results or make retries safe. A subsequent dcc run/attach starts new work rather than reconnecting to the original exec.

Current reset/recovery constraints matter when evaluating changes:

- Graceful stop can remain blocked by active work or stale records and returns after its bounded wait without guaranteeing removal.
- stop-now sends termination signals to registered wrapper PIDs and depends on working bookkeeping.
- stop --kill bypasses bookkeeping for a running container. Replacement clears container-private command state; declared persistent data survives, while container-local changes are lost.
- A never-started container is outside the running-only dcc stop lookup and requires explicit Docker removal.
- CLI acknowledgment would introduce additional abandoned-reservation cases even when wrappers exit normally. Automatic expiry would be a cancellation policy, not proof that work or its caller has finished.

## CLI-disconnection detection: deferred

User decision, 2026-11-17: note the possibility of detecting CLI disconnection before command launch, but defer further investigation of that mechanism. This deferral applies to disconnection detection, not the two confirmed teardown races; T-0113 remains open.

A control connection whose closure identifies an abandoned pending launch could let the supervisor initiate cleanup promptly rather than waiting for the orphan timeout. However, an in-container Docker exec readiness helper may outlive the original CLI; helper PID liveness alone does not establish that the CLI is still connected. A design would need to determine which connection or helper lifetime actually tracks the caller, including abrupt CLI death and remote Docker transport behavior. A network failure may remain half-open without promptly observable EOF, so immediate disconnect detection cannot yet justify removing the timeout fallback.

No control-channel change, heartbeat/lease mechanism, or timeout removal is selected or authorized here. Preserve current startup timeout behavior while this investigation is deferred. General crash/reset behavior remains relevant when evaluating race fixes; implementing prompt disconnection detection is not an acceptance condition for them.

Created-but-unstarted container recovery is tracked separately in [T-0114](0114-stop-unstarted-container.md).

## Post-launch interactive disconnection experiments

User-requested local experiments completed on 2026-11-17: 35 completed trials with
real PTYs, durable and one-shot runtimes, an input-waiting probe, finite work,
tmux/screen, and the Codex login TUI. See the linked interactive-disconnect report
for methods, artifacts, corrected harness attempts, and limits.

- Ctrl-D/Ctrl-C ended the probe and cleared its registration; durable containers
  stayed running and one-shot containers were removed.
- Docker detach, terminal closure, killing dcc alone, and killing the host Docker
  exec client left the input-waiting job alive and registered in both modes.
  One-shot survival was verified after an additional 12-second wall-clock wait,
  with the original CLI already returned. This is not stale bookkeeping: the job
  was still alive, and the pre-registration orphan timeout no longer applies.
- tmux/screen own detach ended the registered client/wrapper: durable servers/jobs
  survived and were reconnectable, but one-shot teardown killed the entire mux
  server/job. Docker detach or terminal loss left the mux client registered;
  reconnection to its existing job worked in both modes while the container lived.
- A separate exec could explicitly terminate the disconnected probe; registrations
  cleared and one-shot containers cleaned up. Finite work reached completion and
  cleaned up despite Docker detachment. A disconnected interactive job can instead
  wait indefinitely for input unless reconnected or terminated.
- Codex login/onboarding processes survived Docker detach and terminal loss in
  both modes. Authenticated Codex work was not tested.

Do not infer that a disconnected client means its running job should be reclaimed.
Keep the deferred pre-command detection investigation distinct from this completed
post-launch observation phase. The current container-owned direction remains
preferred; both teardown races and the task remain open.

## Remaining research

- Find how to serialize registration, active-state observation, and the transition to draining within the current container-owned model.
- Investigate a completion mechanism that prevents wrapper teardown from racing Docker result delivery without requiring a surviving CLI acknowledgment. Establish what process exit/liveness observations actually guarantee before relying on them.
- Preserve automatic cleanup after normal wrapper exit when the CLI disappears; examine abrupt wrapper death, stale records, concurrent commands, and stop/reset behavior explicitly.
- Use the existing deterministic checkpoints to assess candidates; avoid treating a fixed sleep as a completion guarantee.

## Acceptance and verification

- Preserve the two deterministic interleavings as meaningful regressions, rather than relying only on probabilistic repeated commands. The checkpoint experiments establish diagnosis, not correctness of a future fix.
- Verify successful and nonzero short commands preserve exact status and output, including commands completing between supervisor polling intervals.
- Verify concurrent commands cannot be terminated by stale state or registration versus drain races.
- Verify CLI crash/disconnection, interruption, failed exec launch, abrupt wrapper death/stale records, and initial registration ordering for the selected fix. Preserve container-owned cleanup; CLI acknowledgment is not the selected direction.
- Preserve one-shot cleanup, durable/--keep behavior, stop behavior, and command registration safety.
- Run applicable repository checks and Docker regressions; assess compatibility/version requirements and document any protocol change.

Relevant source: src/supervisor.rs exec_script registration, EXIT cleanup and supervisor active_count/drain loop; src/exec.rs execute_in_runtime; Docker foreground exec handling.

Original fixture: /tmp/dcc-repro-0111-8_m65wh4. Original events: /tmp/dcc-repro-0111-codex-events.jsonl. Deterministic reproduction script: /tmp/dcc-0113-races.py; artifacts: /tmp/dcc-0113-races-5e1__ayh. Temporary artifacts may expire; the linked quality record preserves the method and results.
