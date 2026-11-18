# T-0102: Docker Upgrade

2026-11-16. Amazon Linux 2023 aarch64. User authorized newer Docker after
T-0101 exposed the Engine 28+ forwarding requirement.

Amazon Linux's configured repository offers only Engine 25.0.18. Installed
Docker's official stable ARM64 static Engine/CLI 29.9.3 in `/opt/docker/29.9.3`,
with explicit docker-init and docker-proxy paths. Installed official Buildx
0.39.0 under `/usr/local/lib/docker/cli-plugins/docker-buildx`, verified against
the release checksums. Static Engine archive fetched via official HTTPS.

A systemd drop-in selects the new dockerd; `/usr/local/bin/docker` selects
the new CLI. Original RPM binaries remain intact. Existing distro containerd
2.3.7 and runc 1.4.3, sysconfig options, Docker socket permissions, data directory,
and overlay2 remain in use. No containers existed at daemon restart. No remote
API enabled. Static Engine/CLI and Buildx require manual security updates.

## Verification

- `docker version`: client and daemon 29.9.3; successful connection as ec2-user.
- `docker buildx version`: 0.39.0.
- Docker service active and enabled at boot.
- `cargo test --locked --test docker_smoke forwarded_port_reaches_container_loopback_service -- --ignored --exact --nocapture`: **passed**, 1 test in 9.67 seconds.
- No test containers remained afterward. No product source changed.

Log: `/tmp/dcc-docker-upgrade/forwarding.log`. Host install/rollback notes:
`/opt/docker/README.md`. To revert binary selection, stop Docker/socket, remove
only this service drop-in and CLI symlink, reload systemd, start Docker/socket.
Keep all image data intact; assess downgrade compatibility after future workloads
or migrations. This is a focused rerun, not a full smoke-suite rerun or complete
T-0097 platform qualification.
