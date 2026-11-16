# T-0101: Docker Smoke Run on Main

Date: 2026-11-16. Requested execution completed; suite result **FAIL**.

Revision: local main `54a8d7185db1e71400bda145d7d5ac40690c1001`.
Fetched origin/main `c68b82c08711dff67434b97ad013efafcd67ee5f` has the same
product source; the local extra commit only records host setup.

Environment: Amazon Linux 2023.12, aarch64, Docker Engine 25.0.18,
overlay2, cgroup v2. Installed GCC 11.5 and rustup minimal stable Rust 1.100.0
because the fresh host lacked build tools. Ran as ec2-user using `sg docker`.
User explicitly authorized smoke-test container launches for this task.

## Results

Executed the five commands from `.github/workflows/ci.yml` serially, adding
`--locked` to enforce the committed dependency resolution:

| Command | Result |
| --- | --- |
| `cargo test --locked --test cli_flags strict_accepts_valid_config -- --ignored --exact --nocapture` | 1 passed |
| `cargo test --locked --test cli_flags exec_runs_direct_command -- --ignored --exact --nocapture` | 1 passed |
| `cargo test --locked --test cli_flags run_executes_named_command -- --ignored --exact --nocapture` | 1 passed |
| `cargo test --locked --test docker_smoke -- --ignored --nocapture --test-threads=1` | 32 passed, 1 failed |
| `cargo test --locked --test tls_oci_docker_smoke -- --ignored --nocapture --test-threads=1` | 2 passed |

Total: **37 passed, 1 failed**. Logs are local transient artifacts at
`/tmp/dcc-smoke-main-54a8d71/{strict,exec,run,docker,tls}.log`.

## Failure Diagnosis

`forwarded_port_reaches_container_loopback_service` failed at
`tests/docker_smoke.rs:241`: empty response instead of `container-response`.
The test does not surface the child stderr before this assertion. A separate
minimal temporary fixture using the unchanged compiled binary, Debian bookworm,
root user, and one forwarded port built successfully, then `dcc exec /bin/true`
exited 1 with:

> forwarding requires Docker Engine 28+ for safe localhost publication; upgrade the daemon

This matches `src/runtime.rs`'s explicit Engine floor. The Engine 25 host cannot
qualify forwarding; no guard or test was weakened and no daemon upgrade was
performed during this run. Qualify on Engine 28+ to resolve this limitation.

## Cleanup and Limits

Test fixtures and diagnostic container/image names were cleaned using their exact
identities. `docker ps -a` was empty after the suite and diagnostic cleanup.
CLI flag tests leave three fixture-specific built images; base-image/build caches
also remain. No broad prune was performed. No product code changed, push or release.
This run does not qualify Docker Desktop, other distro images, or the remaining
T-0097 platform matrix; T-0097 remains Needs verification.
