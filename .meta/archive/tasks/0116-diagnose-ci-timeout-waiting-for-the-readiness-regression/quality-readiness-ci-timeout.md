# T-0116: Diagnose readiness smoke startup-hook timeout

Date: 2026-11-17. User requested diagnosis, not implementation.
Checkout: b751559 (product fix from 3f07f42 unchanged).
Environment: Linux aarch64, Docker Engine 29.9.3.

## Finding

The regression in tests/docker_smoke.rs disables updateRemoteUserUID while its
non-root startup hook writes /workspace/bootstrap-entered in the bind-mounted host
fixture. Debian's dev user is UID 1000. The fixture belongs to the test runner,
which need not be UID 1000. With different UIDs, dev cannot create the marker.

The shell payload uses `touch ...; while ...` without fail-fast handling. After
touch reports Permission denied, it enters the release-file loop. The test waits
for the missing entered marker before it will create that release file. Neither
side can progress, and ChildGuard reports its generic 15-second timeout. Hook
output remains in /run/dcc/hook.log while bootstrap is unfinished; the test's
captured CLI stderr does not expose it at this point.

This is a fixture permission assumption, not evidence that readiness notification
or the T-0112 control-user correction is wrong. Merely increasing the deadline
will not resolve the permission failure.

## Local confirmation

Built the existing docker_smoke test binary with cargo test --locked --test
docker_smoke --no-run --message-format=json; no source changes.

- Ran exactly readiness_waiters_match_supervisor_user_during_bootstrap as root
  through sudo: host fixture UID 0, container dev UID 1000. It reproduced the exact
  tests/docker_smoke.rs:164 panic, `timed out waiting for startup hook`, in 15.66 s.
- During the wait, inspected the isolated test container. /workspace was owned by
  UID/GID 0 with mode 0755. The configured user reported uid=1000(dev). hook.log
  contained `touch: cannot touch '/workspace/bootstrap-entered': Permission denied`.
  The hook process was alive inside its release-file polling loop; wait-ready was
  also alive, awaiting bootstrap. Host stat confirmed fixture UID 0/GID 0/mode 755.
- Re-ran the identical binary/test as the normal host UID 1000, using the local
  sudo Docker wrapper. It passed all its root/non-root cases in 20.77 s.

Temporary evidence:
- /tmp/dcc-ci-root-regression.log
- /tmp/dcc-ci-root-hook-evidence.txt
- /tmp/dcc-ci-matching-uid-regression.log

The failure was reproduced without needing a slow or overloaded runner.

## CI attribution and limit

The workflow uses ubuntu-latest. GitHub's official runner-images announcement
[documents UID 1001 on standard runners](https://github.com/actions/runner-images/issues/10936).
That is consistent with this fixture failing on CI while passing locally with UID
1000. The supplied panic and local reproduction were examined; the actual failed
CI run's full logs and numeric ownership were not independently inspected.

## Correction identified during diagnosis

Make the fixture's coordination files independent of host/container UID equality.
Prefer a dedicated test-only shared coordination directory with explicit access
for both users, updating entered/release/failure paths accordingly. This preserves
the deliberate non-root UID and isolates the permission grant to test scratch
space. Do not make the project's general workspace or runtime FIFOs world-writable.
Alternatively, normal UID remapping aligns ordinary non-root runner UIDs, but its
root-UID collision policy means removing updateRemoteUserUID=false alone does not
make the dev test portable to all root-run environments.

Make marker-creation errors fail the hook immediately (for example, fail-fast or
an explicit checked touch), and improve timeout diagnostics to reveal the hook
log/container user and ownership. Preserve deterministic waiter-before-bootstrap
coverage. Verify the correction with matching and mismatched UIDs and root and
non-root supervisors. The correction was subsequently implemented and verified in [T-0117](../0117-readiness-fixture-uid-portability/readiness-fixture-uid-portability.md); see its [verification record](../0117-readiness-fixture-uid-portability/quality-readiness-fixture-uid-portability.md).

Isolated diagnosis containers were removed by the existing test fixture cleanup.
No production or test source changed during this diagnosis. The subsequent test-only
correction belongs to T-0117. T-0113/T-0114/T-0115 are unaffected.
