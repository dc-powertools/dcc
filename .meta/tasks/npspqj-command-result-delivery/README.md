# npspqj: Preserve command results across one-shot teardown

## Identity And Authority

- ID: npspqj
- Revision: r1
- Source and date: User request, 2026-11-18, to split deferred work from T-0113 before closing and archiving it.
- Split from: [T-0113](../../archive/tasks/0113-fast-command-teardown-race/README.md), closure revision r10; implementation/evidence revision r9.
- Status: Parked
- Dependencies: T-0113 (Done); no unfinished hard dependency.
- Route / risk: Decide / High
- Approval or blocker: Work remains deliberately deferred. This request authorizes task separation and archival, not product implementation or selection of the previously unselected result journal.
- Next safe action: When resumed, evaluate post-exit result-delivery designs against the existing deterministic delayed-client evidence, then record the chosen scope and verification before implementing.
- Results: Deferred work captured; no product change or new execution qualification.

## Outcome, Scope And Acceptance

Preserve the real command status and exact stdout/stderr when a delayed Docker exec
client retrieves results after the wrapper exits and a one-shot container is removed.
Process exit, wrapper retirement and Docker result retrieval are distinct boundaries;
the completed T-0113 lifecycle lock does not solve the last one.

Primary scope:

- Reproduce the post-exit hazard using explicit client/API checkpoints, comparing
  retained containers with automatic removal; preserve correlation to the exact
  invocation and container generation.
- Compare result transport/journaling and container retention alternatives. No journal,
  mandatory CLI acknowledgment, automatic replay or extra grace delay is selected.
- Define exact status/output delivery, partial-result and transport-failure semantics,
  disconnected-client cleanup, storage/retention/backpressure and trust rules for the
  chosen approach. Preserve container-owned cleanup and the verified admission lock.
- Assess baked-script/host compatibility, packaging and required version migration.

A separately deferred research thread from T-0113 is also retained here: pre-command
CLI-disconnection detection. Investigate only when explicitly resumed; it is not a
prerequisite for the result-delivery fix. A helper PID alone cannot establish caller
connectivity, especially with remote/half-open transports. No heartbeat, lease,
control-channel change or removal of the existing orphan timeout is authorized by
this split. Session reattachment stays with T-0115; unstarted-runtime removal stays
with T-0114.

Observable acceptance for a future result-delivery implementation:

- Force actual wrapper exit/removal before resuming a paused Docker client; recover
  correlated status and exact stdout/stderr without Docker diagnostics mixed into
  user stderr. Cover zero/nonzero status, slow readers, large/binary/empty output,
  and supported PTY behavior. Small fast-command runs are insufficient evidence.
- Exercise client/wrapper death around publication and retrieval; finite disconnected
  work cleans up and live work is not reclaimed. Cover stale/cross-generation results,
  signals, launch/bootstrap failure, user exits 252/253, and result-sink exhaustion.
- Preserve one-shot/durable/keep and stop/reset behavior, root/non-root ownership,
  UID remapping and admission safety; never replay ambiguously executed user code.
- Run deterministic regressions, applicable full repository checks and live Docker
  smokes; independently review lifecycle, transport/storage trust and compatibility.

## Evidence And Handoff

Canonical historical evidence stays in the archived T-0113 bundle:

- [Design review and delayed-client CLI/API probes](../../archive/tasks/0113-fast-command-teardown-race/fast-command-teardown-race.md#design-review-and-further-evidence-2026-11-17-r7).
- [Deterministic result-delivery regression criteria](../../archive/tasks/0113-fast-command-teardown-race/fast-command-teardown-race.md#deterministic-regression-criteria).
- [Deferred pre-command detection analysis](../../archive/tasks/0113-fast-command-teardown-race/fast-command-teardown-race.md#cli-disconnection-detection-deferred).
- [Verified startup/admission implementation and limits](../../archive/tasks/0113-fast-command-teardown-race/execution-r9.md).

The Linux Docker 29.9.3 probes established that a paused real CLI returned the
command's exit 7 with a retained container, but returned 1 / No such exec instance
after auto-removal. The API likewise returned ExitCode 7 for retained metadata and
404 after removal. These observations do not qualify every Docker version, prove
indefinite retention, or establish slow-reader output completeness. The linked
historical record preserves the method if temporary scripts/results have expired.
