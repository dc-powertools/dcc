# T-0112: Readiness FIFO ownership fix

## Scope and implementation

User authorized implementation and a local commit on 2026-11-17 (r2).
`docker::control` now omits the root override only for wait-ready and wait-hooks.
Docker exec therefore inherits Config.User, which is the same configured user used
by the supervisor at container creation. Both sides own the waiter FIFOs under the
same UID without chmod, chown, extra inspection, or broader access. Snapshot,
configuration verification, mode, stop, and other controls retain root execution.

This is a CLI-only correction. Baked supervisor assets, control verbs, registration
semantics, snapshot format, and exit codes remain unchanged. Existing compatible
images work without a rebuild; no protocol/version bump is required. Already
blocked root-owned waiters in running containers are not repaired retroactively;
stop and recreate those containers with the fixed CLI.

T-0113 teardown work remains independent and unimplemented. This change does not
claim to fix every possible status 137.

## Regression evidence

- Added a Docker-boundary test that observes both initial and reused controls:
  wait-ready/wait-hooks inherit the container user; snapshot/verify-config retain
  explicit root. It failed on the original code and passed after the correction.
- Added a live Docker regression for dev and root supervisors. A host-controlled
  hook blocks bootstrap until the test observes the registered FIFO(s), verifies
  matching ownership and unchanged 0644 permissions, and verifies bootstrap has
  not completed. This deterministically exercises notification rather than the
  status-file fast path. The original code failed the ownership assertion.
- Live coverage includes two cold named runs separated by stop, durable start,
  concurrent reuse registering a second wait-hooks waiter during bootstrap, and a
  failed hook reporting its name and output. Launch snapshot mode remains 0600
  and root snapshot access succeeds. Snapshot ownership is the host UID; matching
  container UIDs may read it, so the regression does not incorrectly assume that
  only root can read every snapshot on every host.
- Focused live regression passed all root/non-root cases in 21.96 seconds.
- Re-ran the unchanged T-0111 fixture/image with a forwarded port and sleep hook:
  durable start (2.544 s), named run (0.466 s), stop, cold named run (5.442 s),
  cold --keep named run (2.730 s), stop; every invocation exited 0. The existing
  image ID was sha256:b2e170ec0885b966c6c2c7873bd5e02b828a181a49c859cd3d4802745c40cab3.

## Verification and review

Environment: Linux aarch64, Docker Engine 29.9.3. Docker was accessed through a
local sudo wrapper on PATH; the Rust tests and dcc itself ran as the host user.

- cargo fmt --check: passed.
- cargo clippy --all-targets -- -D warnings: passed.
- cargo test --locked --quiet: passed, 572 unit and 125 integration tests.
- cargo build --locked --quiet: passed.
- Focused Docker regression: passed.
- Full Docker smoke suite: passed, 34/34 in 150.94 seconds, using
  `PATH="/tmp/dcc-0112-docker-bin:$PATH" cargo test --locked --test docker_smoke -- --ignored --test-threads=1`.
- git diff --check: passed.

Focused diff/security review: only two readiness verbs lose root privileges;
other control callers and their protected data access are unchanged. No new
permission grants, dependencies, supervisor edits, or protocol changes. Existing
ChildGuard and DockerFixture cleanup bound failure cases and remove test containers
and images. Linux is verified here; this does not close the separate Desktop/macOS
persistence qualification gate in T-0108.

Result: T-0112 complete. No product changes for T-0113.
