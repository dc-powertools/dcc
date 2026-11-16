This local Feature is a deterministic fixture for persistence tests. It downloads
nothing. Copy the fixture into a temporary workspace's `.devcontainer` directory,
use `Dockerfile` with `containerUser: "fixture"`, and reference `./feature`.

The `imageVersion` option selects `v1` or `v2`. Each version installs an executable
payload under `~/.persistence-fixture/packages`, an empty `scratch` directory, and
versioned user defaults under `preferences`, `sessions`, and `cache`. All are owned
by the remote user. The installer relocates both exclusions in the installation
layer to `/opt/dcc-local/persistence`, outside the persistent root.

`/usr/local/libexec/persistence-fixture-assert-ready` checks populated and empty
excluded directory links, user defaults, executable payload, and write permissions.
Tests can call it from build-preparation and runtime hooks to verify ordering.
