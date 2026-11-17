# T-0117: Fix readiness regression UID portability and failure diagnostics

- Authority: user task-recording and pending-documentation commit request 2026-11-17; revision r1.
- Scope: future test-fixture correction and verification; implementation has not started.
- Diagnosis: [T-0116](../quality/0116-readiness-ci-timeout.md).
- Related implementation: [T-0112 FIFO ownership fix](0112-readiness-fifo-ownership.md). The production fix remains valid; this task addresses its regression fixture.

## Problem and outcome

readiness_waiters_match_supervisor_user_during_bootstrap disables UID remapping
but asks container dev (UID 1000) to write a startup marker into the host-owned
workspace. On a different host UID, marker creation fails with Permission denied.
The hook then waits for a release file that the test will not create until it
observes that missing marker. This produces a misleading 15-second startup-hook
timeout. The exact panic was reproduced with host UID 0 and container UID 1000;
the identical test passed with host UID 1000. Standard CI runner UID 1001 is
consistent with the reported failure; the actual CI run's full logs were not read.

Make the fixture independent of host/container UID equality, and expose genuine
startup failures promptly without weakening the readiness-notification regression.

## Implementation constraints

- Prefer a dedicated test coordination directory with explicit access for both
  the host and configured container user. Update entered/release/failure markers
  and cleanup together. Limit any extra permissions to disposable test scratch
  space; preserve workspace, FIFO, and snapshot permission boundaries.
- Retain meaningful non-root supervisor coverage. Enabling UID remapping alone
  is insufficient for root-run portability because remapping dev to host UID 0
  collides with the existing root account and is skipped by policy.
- Make marker creation fail the hook immediately rather than entering its release
  loop after failure. Do not increase timeouts to conceal permission errors.
- Improve the test's early-exit/timeout diagnostics so failures reveal bounded
  relevant hook output and UID/ownership information instead of only a generic
  missing-file timeout. Collect diagnostics before fixture cleanup; avoid leaking
  arbitrary snapshots, environment values, or unrelated container data.
- Preserve the gate that proves waiter registration precedes bootstrap completion,
  matching supervisor/FIFO ownership, failed-hook reporting, concurrent wait-hooks
  reuse, and protected snapshot access.

## Acceptance and verification

- Run the corrected live regression with matching and mismatched host/container
  UIDs, including a root-run host and non-root/root supervisors. Exercise a CI-like
  non-root host UID different from 1000 where practical, and record the actual
  numeric IDs and directory permissions tested.
- Verify both successful bootstrap and intentional hook failure, cold named run,
  stop then cold run, durable start, and concurrent attachment during startup.
- Check that an unwritable coordination marker produces a useful prompt failure,
  without a full marker-wait timeout or deadlock.
- Keep all test processes, containers, fixture files, and images cleaned up on
  assertion failures as well as success.
- Run repository formatting, Clippy, regular tests, build, and relevant Docker
  smoke coverage; inspect CI results when available and state platform limits.

Relevant source: tests/docker_smoke.rs readiness regression and ChildGuard waiting
helpers; tests/common/mod.rs fixture construction; UID remap policy for comparison.

First next step: implement isolated coordination storage and failure diagnostics,
then verify under the UID combinations that distinguished failure from success.
