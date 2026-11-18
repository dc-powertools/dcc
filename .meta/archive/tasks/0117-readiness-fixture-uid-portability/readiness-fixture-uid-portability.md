# T-0117: Fix readiness regression UID portability and failure diagnostics

- Authority: user orchestration and implementation request 2026-11-17; revision r2.
- Scope: implement and verify the test-fixture correction, complete independent review, and create a local task-scoped commit.
- Diagnosis: [T-0116](../0116-diagnose-ci-timeout-waiting-for-the-readiness-regression/quality-readiness-ci-timeout.md).
- Related implementation: [T-0112 FIFO ownership fix](../0112-readiness-fifo-ownership/readiness-fifo-ownership.md). The production fix remains valid; this task addresses its regression fixture.

## Completion

Implemented and independently reviewed on 2026-11-17. Isolated coordination storage
removes the workspace UID assumption; marker creation is fail-fast and diagnostics
include bounded hook and ownership details via private file-backed stderr. Both
readiness tests pass with actual host UIDs 1000, 0 and 1001. All 35 Docker smoke
tests, 572 unit and 125 integration tests, formatting, Clippy and build passed.
No production source or supervisor protocol changed. See
[verification and review](quality-readiness-fixture-uid-portability.md).

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

Next action: stop; fixture correction and all required verification are complete.

## Execution ownership (r2)

- Root: catalog/brief/quality records, integration, host UID matrix, full required
  checks, final review and commit. Existing primary T-0108 is preserved as pending
  broader work; only T-0117 is being implemented.
- implement_0117 (complete): sole code writer for tests/docker_smoke.rs (tests/common/mod.rs
  only if necessary); focused fixture/diagnostics/negative coverage; no product
  source edits or commits. Reports checks and hands off live Docker runs to root.
- review_0117 (complete; no blocking findings): read-only independent review of UID coverage, permissions,
  diagnostics bounds/privacy, cleanup and final diff. No Docker runs or commits.
- Shared worktree, disjoint ownership; after interruption inspect status and agent
  messages before reassigning work. Agents must not touch shared task records.

Verification plan: formatting, Clippy, regular suite, build; focused live Docker
regressions including marker-denied failure; actual test process UIDs 1000, 0 and
1001 if local permissions permit; full existing Docker smoke suite after review.
