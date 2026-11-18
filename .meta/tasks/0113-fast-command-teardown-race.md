# T-0113: Diagnose and fix fast-command teardown race

- Authority: user task-recording request, subsequent design/reproduction request, explicit task-update requests, and user preference for the current approach after crash analysis, and evidence/design review with task update, 2026-11-17; revision r7.
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

## CI one-shot registration failure follow-up

On 2026-11-17 the user reported one_shot_container_drains_after_command_without_grace
failing because dcc exited 137 before its ready marker. [T-0119 diagnosis](../quality/0119-one-shot-registration-ci.md)
reproduced the same boundary with the actual dcc 0.3.1 CLI and exact test command,
using temporary checkpoints: supervisor sampled n=0, wrapper registered/ARRIVED,
then supervisor resumed before user-command execution. The wrapper had a real
active record but no ready file; stale n=0 plus ARRIVED caused shutdown. CLI
returned 137; Docker recorded foreground exec 137 followed by container exit 0.
Readiness and snapshot controls succeeded first, with no oom event in the captured
sequence. This specifically confirms the registration/stale-count race can explain
the new CI symptom; no CI-run event logs were available to attribute that exact
remote execution conclusively.

The test's wording calls its marker command registration, but the user command
writes it after actual wrapper registration and wait-hooks. The root-user test is
not subject to T-0116's non-root marker permission problem. T-0117 and the patch
version bump did not implement either T-0113 race fix. Keep T-0113 open under the
current container-owned design preference; product implementation remains pending.

## Design review and further evidence (2026-11-17, r7)

User authorized evaluation and task updates only: no implementation, commit, push,
or release. Initial dirty state was this task, the task catalog, and untracked
0119-one-shot-registration-ci.md; preserve that prior work. This review reads the
current checkout 1197c2c and retained evidence, and keeps the task open.

### What the experiments establish

Source inspection agrees with both diagnoses. `active_count` is a filesystem scan,
not an atomic snapshot; the wrapper checks STOPPING separately from registration.
The supervisor uses its cached count when checking ARRIVED, STOPPING, and mode.
Thus checking ARRIVED first, counting twice, or adding a second STOPPING check
without serialization only moves a scheduling window. Graceful stop and the
orphan-reaper decision need the same admission boundary as ordinary one-shot drain.
Mode promotion to durable must also participate, or race with committed teardown.

Reran `/tmp/dcc-0113-races.py`: both cases again returned exec 137, container 0,
OOMKilled=false. New artifacts: `/tmp/dcc-0113-races-7hivlrge/results.json` and script
copies. These use the earlier baked test image; current source retains the relevant
logic. T-0119 independently covers the actual 0.3.1 CLI and pre-user-command marker
boundary. Neither checkpoint reproducer attributes the original uninstrumented
failure or the particular remote CI run conclusively, nor estimates race frequency.
The exit-trap checkpoint proves deregistration precedes wrapper exit, not that
waiting for process exit proves Docker result delivery. No output truncation was
observed in these small-output reproductions; output completeness remains a separate
acceptance criterion.

A further isolated experiment establishes that last distinction with Docker 29.9.3
(client commit 5457199, engine 70efc0e), Linux aarch64:

- A synthetic shell PID 1 waits for the exec shell's PID to disappear using
  `kill -0`, then exits. This is a disposable lifecycle probe, not a candidate
  product implementation. The exec prints `exact-output` and `exact-error`, exits 7.
- Pause the real host `docker exec` client with SIGSTOP after the exec has started
  but before releasing its command. Release through a separate exec; wait for
  container stop/removal; resume the client with SIGCONT.
- With the stopped container retained, Docker CLI returns 7 with exact output.
  With `--rm`, it returns 1, stdout is intact, and stderr contains the command's
  bytes followed by `Error response from daemon: No such exec instance`.
- A separate Engine API probe consumes the complete attached output stream, delays
  exec inspection until container stop/removal, and observes ExitCode=7 with HTTP
  200 for the retained container versus HTTP 404 after auto-removal.

Scripts: `/tmp/dcc-0113-post-exit-cli.py`, `/tmp/dcc-0113-post-exit-api.py`.
Results: `/tmp/dcc-0113-post-exit-cli-68m9ofnh/results.json` and
`/tmp/dcc-0113-post-exit-06qx3x9z/results.json`. All probe containers removed.
The pauses force allowed client scheduling, rather than relying on an arbitrary
sleep to make a race likely. They establish a post-exit result-retrieval failure,
not that all Docker versions behave identically, that retained results last forever,
or that these probes exercise slow-reader output truncation.

The matching [Docker CLI source](https://github.com/docker/cli/blob/5457199/cli/command/container/exec.go#L181-L201)
retrieves status by ExecInspect after streaming finishes. Process exit therefore
cannot establish that the client has retrieved status. This is a distinct boundary
from both user-command completion and the wrapper's EXIT trap. Waiting for a daemon
exec-die event alone likewise cannot prove that a delayed client has queried status.

### Options within container-owned cleanup

| Option | Benefit | Limit / disposition |
| --- | --- | --- |
| Reorder reads, recount, or add grace sleeps | Small change; can reduce frequency | No atomic admission boundary or completion proof. Reject as correctness solution. |
| Shared lifecycle lock around admission and drain commitment | Prevents new accepted work from invalidating idle state | Necessary admission repair, but a wrapper must not remove its last lifetime protection before exiting. Does not solve Docker result retrieval. |
| Supervisor observes wrapper death, using stable process identity or kernel lifetime locks | Retains cleanup when CLI disappears; can reclaim records after wrapper SIGKILL | Closes early deregistration only if protection lasts through actual process death. PID reuse, zombies, inherited FDs and surviving children require explicit handling. Still fails delayed status retrieval with auto-removal. |
| Supervisor launches and waits for commands as children | Owns authoritative wait status, avoids stale wrapper-only accounting | Larger execution/PTY/signal change; parent wait does not prove host output delivery. Needs a result transport and frontend-death policy. |
| Stop container but retain it for Docker exec status lookup | Local delayed-client comparison passes | Changes auto-removal/reset/name-reuse behavior; Docker exec metadata is garbage collected. Finite retention is not an unlimited scheduling guarantee; a reaper racing readers reintroduces the problem. Diagnostic option, not sufficient final design. |
| Container-owned result journal outside container-private state | Cleanup can finish without surviving CLI; result survives auto-removal | Requires correlated status/output transport, retention, storage/backpressure and trust rules. Recommended completion direction for qualification. |
| Mandatory CLI completion acknowledgment | Directly orders teardown after the caller gets Docker result | Adds abandoned reservations and caller-dependent cleanup. Remains unselected per user preference. |

### Recommended design to qualify, not an approved implementation

Use a serialized container-owned lifecycle plus results whose lifetime is independent
of container removal. Separate admission, execution liveness, and result retention:

1. One authority serializes OPEN to DRAINING with registration, mode promotion,
   stop and orphan expiry. Under that authority, accept a command and publish its
   record/ARRIVED atomically with respect to shutdown, or reject before user code
   starts. DRAINING is irreversible for that instance. Accepted commands protect
   lifetime before readiness waits; orphan timeout remains the existing fallback
   for requests that never register. It may win admission, but then work is rejected.
2. Use short critical sections only. Never hold the global lock during user work,
   wait-hooks, shutdown hooks, FIFO I/O or waiting for another process. Prefer kernel
   released locking over a mkdir lock that survives SIGKILL; prove availability and
   permissions in every supported base image. Publish records completely, and install
   cleanup/protection before an interrupt can strand admission. An event-loop owner
   is an alternative, but request publication and owner death still need defined rules.
3. Keep each accepted invocation protected through true wrapper termination; wrapper
   EXIT notification is advisory. Use an instance/request token and stable process
   identity, not just `kill -0` on a reusable PID. A kernel lifetime lock can avoid
   stale records but must not leak into arbitrary user descendants. A child-executing
   supervisor can know wait status directly; a poller cannot `wait` for unrelated
   Docker exec children. Do not reconstruct exit status merely from PID absence.
4. Before permitting automatic teardown, finish publishing the authoritative result
   through a channel surviving removal. Proposed journal scope is a dedicated runtime
   result area on an existing host-backed profile mount, fenced by launch token plus
   unpredictable invocation ID. Keep active bookkeeping container-private. Atomically
   publish completion after status and captured output are finalized; the CLI reads
   that result without sending a lifetime-release acknowledgment. Use exclusive
   creation, safe path handling, ownership and restrictive permissions; commands and
   old container generations must not overwrite another invocation's result.
5. Treat journal results as the command-result authority, rather than blindly
   replacing any Docker failure with a saved status. The frontend must correlate
   exact invocation/instance and distinguish complete, cancelled, launch failure,
   wrapper crash and transport failure. A terminal frame over Docker stdout alone
   can collide with arbitrary bytes and alter TTY behavior; use separate structured
   control data. Full output preservation requires a sequenced stream/spool with
   offsets, so live streaming plus fallback neither duplicates nor drops bytes.
   A status-only file fixes neither output loss nor ambiguous partial execution.

This is more work than a lock-and-PID patch. Recommend qualifying the result channel
before selecting an implementation; do not claim a complete repair after admission
serialization alone. If preserving the exact Docker CLI transport without any result
journal is required, the full guarantee is unresolved under immediate auto-removal
and no mandatory acknowledgment. A finite delay cannot resolve that constraint.

### Behavior and open design questions

- **Concurrency:** accepted work must appear in the same serialized state used to
  commit drain. Late rejected work must never execute. Current 253 is also a possible
  user-command status: do not automatically replay based only on that exit code.
  Any retry needs an unambiguous pre-execution rejection, exact-instance fencing,
  and bounded retry behavior, so reset cannot cause duplicate execution.
- **CLI loss:** does not itself end running work. A finite job should complete,
  publish results and allow one-shot cleanup without acknowledgment. Interactive
  input-waiting work can remain live indefinitely, consistent with the 35 trials.
  Never age out a live job merely because its caller disappeared. Pre-command
  disconnection detection remains deferred; no heartbeat/timeout change selected.
- **Wrapper SIGKILL/crash:** kernel lifetime protection should release, but children
  may remain. Decide whether to track/cancel a command process group or preserve
  today's wrapper-lifetime semantics; process-group escape is possible. A missing
  completion record is an unknown/incomplete result, not success. A manager that
  owns children can capture actual status more reliably, at greater implementation
  cost. Do not reclaim a PID-reused job or signal an unrelated process.
- **Status:** source review also found `wait-hooks || exit $?` runs with `status=0`
  and an EXIT trap using that variable, so a readiness failure may be overwritten
  by 0. A disposable run of the current extracted wrapper template with a wait-hooks
  stub returning 252 confirmed wrapper exit 0 and no user-command execution; artifacts:
  `/tmp/dcc-0113-readiness-status-iiw059xt/results.json`. Launch failures and
  interruption paths need explicit tests for 252, 130,
  143, command-not-found and command exits 252/253. This review does not change them.
- **Output/TTY:** decide spool limits, binary-safe stdout/stderr separation for
  non-TTY, merged PTY semantics, signal forwarding, terminal sizing and slow-reader
  backpressure. Disk-full/spool failure must be explicit, not silently discard
  output or report a complete result. Avoid unbounded secret-bearing logs by default.
  Detached descendants holding output FDs require a defined completion policy.
- **Retention:** result publication is not proof of client receipt. A retention
  policy or user reset may intentionally expire results, but must state that limit.
  Optional consumption acknowledgments may reclaim result files; they must not keep
  the container alive. Local host-backed storage does not automatically solve remote
  Docker/Desktop mount access, host reboot durability, or storage corruption; qualify
  the supported topology and whether fsync is required.
- **Modes:** durable/start/--keep retain the container after command exit; one-shot
  drains when accepted execution and result finalization finish. Mode promotion must
  linearize before drain or fail visibly. tmux/screen own-detach still ends its
  registered client, so one-shot may kill useful mux server work; durable mode is
  required for that use. Ordinary attach still starts new work (T-0115 is separate).
- **Stop/reset:** graceful stop closes admission and waits for accepted execution and
  result finalization in either mode. Stop-now closes admission under the same
  authority before selecting targets, then signals outside the lock; define bounded
  cancellation/escalation for unresponsive jobs, without calling forced output loss
  normal completion. Existing --now is TERM-to-recorded-PIDs, not guaranteed kill.
  --kill/reset remains the escape hatch for a wedged supervisor or result sink, can
  lose unfinished output, and must fence old results from replacement instances.
  Existing stop waits are bounded best-effort; created-but-unstarted removal remains
  T-0114. Do not silently broaden reset into deleting declared persistent user data.
- **Compatibility:** a purely internal synchronization change could preserve public
  verbs/arguments/status semantics. A new invocation/result contract or changed
  252/253 meaning is host-supervisor protocol work and needs at least a minor bump
  under project standards/decision 0004. Journal recommendation therefore targets a
  minor release, with old CLI/new image and new CLI/old image rejection tests. Baked
  supervisor, wrapper and controls must be versioned together; patch drift cannot
  silently require a new host feature.

### Deterministic regression criteria

Future implementation must include executable barriers and state assertions, with
bounded failure timeouts only for harness termination, not as lifetime guarantees:

1. Pause an idle observation before a concurrent admission, and pause registration
   between its stopping check and publication. Force both orders against automatic
   drain, orphan expiry and graceful stop. Either admission wins and work survives,
   or drain wins and user code never runs. Repeat with multiple concurrent commands
   and durable promotion; stop-now must not miss a concurrently accepted target.
2. Pause wrapper cleanup after any advisory completion/deregistration while it is
   still alive. Supervisor must not tear it down. Resume and verify exact 0/nonzero
   results and cleanup, including work wholly between polling intervals.
3. Pause the Docker client before result retrieval, allow actual wrapper exit and
   one-shot removal, then resume. The new design must still deliver the correlated
   status and exact stdout/stderr bytes without Docker diagnostics mixed into user
   stderr. Include slow readers, large/binary output, empty output and PTYs. Merely
   observing exec-die or passing small-output fast runs is insufficient.
4. Kill CLI and wrapper at every admission/result boundary, including lock ownership,
   readiness wait, running child, result write and after publication. Assert no stale
   global lock, false success, cross-generation result reuse or reclaimed live job;
   finite disconnected work cleans up, live interactive work remains recoverable.
   Simulate stale records, PID reuse, unknown process identity and inherited FDs.
5. Exercise failed launch, bootstrap failure 252, user exits 252/253, signals and
   result-sink exhaustion. Confirm status/partial-result distinctions and no automatic
   replay after ambiguous execution. Validate root/non-root ownership and UID remap.
6. Test one-shot, durable, --keep, mux detach, graceful/now/kill, wedged work and reset
   across result publication. Verify old/new protocol rejection and applicable full
   repository checks plus Docker smoke regressions once product code changes.

Verification for this review: source/artifact inspection, two repeated checkpoint
failures, four isolated post-exit probes (API and actual CLI, retained versus removed),
a focused wrapper readiness-status probe, and documentation diff/link checks. No product/test source changed; broad Rust checks
are not evidence of a repaired race and were not rerun for this investigation.

## Remaining research

- Find how to serialize registration, active-state observation, and the transition to draining within the current container-owned model.
- Qualify the recommended independent result journal/transport before choosing implementation. Process-death-only cleanup is now demonstrated insufficient with auto-removal; resolve output/PTY, retention, failure and topology questions without mandatory CLI lifetime acknowledgment.
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
