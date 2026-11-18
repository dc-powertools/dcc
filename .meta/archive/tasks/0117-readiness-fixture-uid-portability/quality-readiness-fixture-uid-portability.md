# T-0117: UID-independent readiness regression

Date: 2026-11-17. User authorized orchestration and implementation (r2).
Base checkout: 37bbe0b. Scope: tests/docker_smoke.rs and task documentation only.

## Implementation

BootstrapSync creates a dedicated disposable coordination directory, explicitly
mode 0777 without a sticky bit, bind-mounted at /dcc-bootstrap-sync. Entered,
release and failure markers all use that mount. Host-owned /workspace permissions
are unchanged, and non-root tests still disable UID remapping. The root-user
case can write markers and a non-root host can remove them during cleanup.
Fixture declaration order ensures container cleanup precedes coordination storage
removal, including assertion failures.

The controlled test hook now uses set -e: denied marker creation fails bootstrap
rather than entering the release loop. Existing waiter-before-bootstrap gating,
root/dev supervisors, repeated cold runs, concurrent wait-hooks reuse, intentional
hook failure, FIFO ownership/mode checks and 0600 snapshot checks are preserved.

BootstrapRun supplies readiness-specific diagnostics for early exits, timeouts and
failed completion. Stderr is a private temporary file, so a surviving Docker
subprocess cannot hang diagnostics by holding a pipe open. Reads are bounded to
the last 4096 bytes. Docker diagnostics collect only configured id, fixed-path
ownership/mode information, bootstrap status and the controlled hook's last 4096
bytes; formatted Docker stdout/stderr are capped at 6144/1024 bytes. No inspect,
environment or arbitrary snapshot dump was added. Existing general Docker helper
calls still have no operation timeout; bounded diagnostic bytes do not imply a
bound on an unresponsive daemon. That broader pre-existing helper behavior was
reviewed and not expanded in this fixture correction.

Added readiness_marker_permission_failure_reports_before_timeout, which places the
entered marker in a 0555 directory and uses non-root dev. It asserts an early CLI
exit, Permission denied, user/ownership context, and no release-file creation.
The normal test exercises the passing path with both root and non-root supervisors.
No production source, supervisor protocol, dependency or version change.

## Independent review and ownership

- implement_0117 owned tests/docker_smoke.rs, supplied the implementation and
  focused negative coverage, and passed check/fmt/Clippy before handoff.
- review_0117 performed a read-only independent review of permissions, UID
  portability, diagnostics/privacy, deterministic gating and cleanup. No blocking
  findings. The inherited Docker-operation timeout limitation above was noted.
- Root owned the task catalog, host UID matrix, full required checks, integration,
  final documentation and commit. No overlapping source writers or delegation
  outside this task. Agent code was frozen before final verification.

## Verification

Environment: Linux aarch64, Rust/Cargo 1.100.0, Docker Engine 29.9.3.

Before the correction, compiled the HEAD readiness test with its dcc path relocated
to a public /tmp binary, using Cargo's existing dependency artifacts. Ran the
unmodified test as an actual UID1001/GID1001 process with supplementary Docker
socket group993. It reproduced the exact startup-marker timeout in 15.77 seconds.
This extended T-0116's root-versus-UID1000 evidence to a CI-like non-root UID.

After the correction, both live readiness regressions passed under:

| Host process UID | Container supervisors exercised | Tests | Time |
| --- | --- | --- | --- |
| 1000 | dev (1000), root (0); dev permission-denied case | 2 passed | 22.11 s |
| 0 | dev (1000), root (0); dev permission-denied case | 2 passed | 17.12 s |
| 1001 | dev (1000), root (0); dev permission-denied case | 2 passed | 17.14 s |

For UID0 and UID1000, used the normal Cargo-built test binary. UID1000 accessed
Docker through the pre-existing local sudo wrapper. UID0 accessed Docker directly.
For UID1001, the private host home is 0700, so compiled the identical edited test
source with Cargo's existing dependencies and CARGO_BIN_EXE_dcc pointed to a copy
of the unchanged product binary in /tmp/dcc-0117-public-tests. Launched via
`sudo setpriv --reuid=1001 --regid=1001 --groups=993`, with a separate disposable
DOCKER_CONFIG and real Docker CLI on PATH. No home/socket permissions, user
accounts or sudoers were changed. Configured dev remained UID1000; root remained
UID0; shared coordination mode was 0777, while the negative subdirectory was
0555. FIFO 0644 and snapshot 0600 assertions passed in the positive matrix.

- cargo check --tests --locked: passed (implementation agent).
- cargo fmt --check: passed.
- cargo clippy --all-targets -- -D warnings: passed.
- cargo test --locked --quiet: passed, 572 unit and 125 integration tests.
- cargo build --locked --quiet: passed.
- Full ignored Docker smoke suite: passed, 35/35 in 145.47 seconds, using
  `PATH="/tmp/dcc-0112-docker-bin:$PATH" cargo test --locked --test docker_smoke -- --ignored --nocapture --test-threads=1`.
- git diff --check and relative documentation links: passed.

Temporary verification scripts and logs:
- /tmp/dcc-0117-public-tests.py
- /tmp/dcc-0117-run-uid-matrix.py
- /tmp/dcc-0117-before-uid1001.log
- /tmp/dcc-0117-after-uid1000.log
- /tmp/dcc-0117-after-uid0.log
- /tmp/dcc-0117-after-uid1001.log
- /tmp/dcc-0117-uid-results.json
- /tmp/dcc-0117-docker-smoke.log

The failing remote CI run was not rerun from this environment; local actual-UID1001
verification covers the diagnosed permission assumption. This does not claim new
macOS/Desktop qualification or fix T-0113's teardown races. T-0114 and T-0115 remain
separate pending work.

Result: T-0117 complete. Test containers were removed; temporary verification
artifacts remain in /tmp. No global host permissions or production behavior changed.
