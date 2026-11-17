# T-0115: Define and implement disconnected-job reattachment

- Authority: user task-recording request 2026-11-17; revision r1.
- Scope: record future design and implementation work; implementation has not started.
- Research: [T-0113 interactive disconnect observations](../quality/0113-interactive-disconnects.md).
- Related: [T-0113 teardown races](0113-fast-command-teardown-race.md), [T-0114 unstarted-container recovery](0114-stop-unstarted-container.md). Resolve implementation dependencies when selecting a design; do not assume either task is a prerequisite for design research.

## Problem and intended outcome

Disconnected interactive jobs can remain alive and registered in durable and one-shot containers. Current dcc attach starts a new shell/command, not a connection to a previous Docker exec terminal. Users can reach a healthy container but have no built-in way to recover the original plain shell or Codex TUI session.

The 35-trial research found that tmux/screen can reconnect to existing jobs while their container lives. Their own detach operations finish the registered client/wrapper, however, and one-shot cleanup can destroy their server/jobs. Codex coverage was its unauthenticated login TUI, not active authenticated work. A live active record does not prove that a job is waiting for input or that no terminal remains attached.

Define the exact user experience and implement a supported way to discover and reconnect to surviving interactive jobs. Candidate forms include a dedicated reattach command, or a doctor command that detects a recoverable job and offers to reattach. These are alternatives to evaluate, not prescribed command names or an approved architecture.

## Design and ergonomics to resolve

- Define what counts as an identifiable session and what can actually be recovered. Distinguish reconnecting to the same job/terminal from starting a fresh shell or restarting an application.
- Evaluate technical mechanisms: Docker exec alone does not currently provide dcc with terminal reattachment. Consider a managed terminal/session backend or supported multiplexer integration without assuming arbitrary existing plain exec sessions can be recovered retroactively. Define opt-in/default behavior and migration/limitations for already-running sessions.
- Decide dedicated command versus doctor-led discovery/offer, possible use of both, and interaction with existing attach/run/exec. Publish concrete help text and end-to-end examples for the chosen interface.
- Define workspace/profile/session selection: no jobs, one candidate, multiple candidates, a still-attached client, a completed job, stale records, and a container that disappears during selection.
- Decide whether to attach concurrently, transfer control by detaching another client, or refuse; do not silently take over an active user's terminal. Define terminal resize, detach keys, signals, Ctrl-C/Ctrl-D, output/history availability, and exit-status semantics.
- Define how a doctor identifies disconnection/recoverability and when it can only report uncertainty. Do not infer idle, waiting-for-input, or disconnected solely from PID liveness or an active record.
- For a doctor interface, define interactive offers and explicit noninteractive/scriptable behavior, including exit codes and any structured output. Diagnosis must not launch a replacement job or terminate existing work implicitly.
- Define durable versus one-shot behavior so detaching a recoverable session does not unintentionally destroy it. Coordinate with T-0113's lifecycle fixes while preserving the user's preference for container-owned cleanup rather than mandatory CLI completion acknowledgments.
- Define how users inspect, cancel, terminate, or reset sessions that cannot be reattached. Keep scope proportional; a doctor-led alternative does not automatically authorize an unrelated broad diagnostic framework.
- Assess backend packaging/dependencies, terminal ownership/access, host/supervisor compatibility and versioning, and supported platforms. Pre-command CLI-disconnection detection remains deferred under T-0113 unless separately revisited.

## Acceptance and verification

- Record the chosen interface, session model, lifecycle semantics, and recoverability limits before implementation. Resolve material user-facing tradeoffs with the product owner.
- Implement same-job reattachment for the explicitly supported session types; clearly explain unsupported existing plain exec sessions rather than claiming recovery or starting duplicate work.
- Add live PTY regressions proving input reaches the original process/session and state survives disconnect/reconnect, not just that a new shell starts.
- Exercise deliberate detach, terminal loss, CLI/Docker-client death, repeated reattachment, root/non-root users, durable and one-shot containers, and competing clients.
- Cover no/multiple candidates, live versus stale records, completed jobs, selection races, noninteractive invocation, stop/reset interaction, and actual cleanup after job completion.
- Include a representative TUI case (Codex where practical, stating authentication/workload limits) alongside a deterministic input-waiting probe. Verify signal forwarding and terminal resize for the supported backend.
- Run appropriate repository and Docker checks; update user documentation and record compatibility/platform limits.

First next step: inspect viable session backends and draft concrete CLI/doctor flows using the disconnect research; no implementation is authorized by this task-recording request alone.
