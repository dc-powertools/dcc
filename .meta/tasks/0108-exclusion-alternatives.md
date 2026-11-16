# T-0108: Exclusion Implementation Alternatives

2026-11-16, r4. User requested recording the investigation, preferring symlink
relocation, and requiring further Codex directory-layout investigation. No product
implementation. Earlier anonymous-volume spikes remain valid supporting evidence.

## Preferred approach: relocate and link

During the Feature installation/image build, put excluded directories outside their
persistent parent, at stable image-controlled paths. After mounting persistence and
before user hooks, create/verify symlinks from the original excluded paths to these
locations. For example:

```text
/home/dev/.codex/packages -> /opt/dcc-local/codex/packages
```

The target resides in the image plus container writable layer. Original image files
are available without copying the entire payload into a new volume. Updates remain
container-local and vanish on replacement. Scratch uses the same behavior, with an
empty installer-prepared target. Preserve the agreed unified exclusion semantics;
no separate scratch/payload configuration mode is needed.

Requirements and caveats:

- A link baked only beneath the persistent parent is hidden when that parent is
  mounted. Prepare/verify it inside the mounted root at runtime.
- Targets must be stable across image versions, so the same durable symlink resolves
  into each container's own image/layer; avoid per-session link changes and races.
- Prepare appropriate ownership and permissions; handle startup and concurrent
  exec/attach consistently. This is not read-only enforcement.
- Safely migrate existing real durable directories and stale links. Do not blindly
  remove contents; preserve/quarantine recoverable old data. Decide exact migration.
- Relocate in the same installation RUN/layer or install at final path where supported.
  Moving earlier-layer payload in a later layer can duplicate bytes in the image.
- Qualify packages for symlink rejection/replacement, canonical paths, sandbox rules,
  updater/pruning assumptions and file watchers. No universal transparency guarantee.
- Anonymous image-initialized volumes remain a fallback candidate where symlinks
  cannot preserve application behavior; do not silently introduce mixed backend
  selection without documenting and testing the final contract.

## Observed Linux evidence

Environment: Docker 29.9.3, classic overlay2, ARM64, Codex 0.169.0 from existing
real-profile image. Disposable containers only, no network or external credentials.

Primary spike under /tmp/dcc-exclusion-alternatives built dcc-codex-symlink-spike,
moving ~/.codex/packages to /opt/dcc-local/codex/packages. Persisted test root was
bind-mounted and its packages symlink installed at runtime.

- codex --version succeeded through the symlink.
- Interactive Codex reached the welcome/sign-in screen and exited successfully.
- A marker written through the link to package storage was absent in the replacement
  container, while the durable link remained valid. Confirms ephemeral writes reset.
- A parallel investigation independently relocated packages and ran daemon bootstrap
  successfully; it copied the invoking package into packages/app-server-daemon through
  the link and started app-server 0.169.0.
- Daemon bootstrap emitted a stale arg0 cleanup Cross-device link warning, also seen
  with the untouched baseline image. Do not attribute that warning to relocation.
- Normal-privilege in-container mount --bind failed with permission denied.
- Test containers removed; no product files or host Docker backend changed. Primary
  spike Dockerfile/build log and daemon README snapshot remain in the /tmp directory.

The official installer explicitly skips release pruning when packages, the standalone
root or releases directory is a symlink. It may warn that old release cleanup was
skipped. Basic startup/bootstrap success does not qualify installation/update,
rollback, daemon migration, sandbox execution or concurrent use. Those remain tests
required before selecting symlinks for Codex in production. macOS/Desktop not tested.

## Other alternatives

| Mechanism | Benefit | Limit |
| --- | --- | --- |
| Native Docker image-subdirectory mount | Exposes an image path without populating a volume. | Engine 28+; containerd image store required; always read-only, conflicting with allowed ephemeral updates. Current overlay2 host cannot test without backend change. |
| Ordinary Docker bind from container-image path | Would expose image tree directly. | Not a valid source namespace: Docker resolves bind sources on daemon host, not inside container. |
| In-container mount --bind | Preserves real directory paths and writable layer behavior. | Additional mount capabilities/namespace setup required; not available with normal container privileges. |
| Host access to storage-driver merged directories | Could expose a rootfs path. | Unsupported Docker internals, races/lifetime coupling, backend-specific and impractical across Desktop VM. Reject. |
| Overlay over a read-only image mount | Could provide copy-on-write. | No simple portable unprivileged Docker run composition; extra mount setup/capabilities and complexity. Not preferred. |
| Shared volume keyed by image digest | Copy once per image. | Writable sharing persists updates across containers and changes isolation semantics; read-only loses ephemeral updates. |
| Fresh anonymous volume | Real directory paths and writable ephemeral updates; earlier Linux tests passed. | Copies payload per container and requires volume lifecycle cleanup. Fallback candidate. |

Image mounts can use an immutable local image reference and image-subpath. Documented
requirements, not tested on current classic storage backend. Do not change the host
storage backend solely to qualify this option. Dockerfile RUN --mount from an image
is a build-only facility and does not provide equivalent runtime mounts.

## Codex correction: runtime directories are not yet fully classified

Current upstream daemon documentation says CODEX_HOME/app-server-daemon/settings.json
holds remote-control launch preferences and updater settings alongside PID/lock/log
files. The earlier proposal to exclude the whole app-server-daemon/ as scratch is
unsafe without an explicit decision to discard those preferences. Absence from a
fresh image is not evidence that every future file there is disposable.

Keep packages/app-server-daemon/ classified as installed payload. Re-investigate the
separate runtime app-server-daemon/ directory for the supported Codex version(s):

- Confirm settings.json and all other user state versus process-local records.
- Determine supported path overrides or persistence layout preserving preferences
  while resetting stale process state. Do not select individual writable file binds
  without checking atomic-save and locking behavior.
- Verify runtime daemon bootstrap, update, rollback, migration, sandbox paths, and
  behavior when package roots are symlinks, including skipped pruning.
- Review app-server-control/, tmp/ and .tmp contents rather than assuming names imply
  disposable state. Account for SQLite sidecars and user-selected skills/plugins.

## Sources

- https://github.com/openai/codex/blob/main/scripts/install/install.sh
- https://github.com/openai/codex/blob/main/codex-rs/app-server-daemon/README.md
- https://docs.docker.com/engine/storage/image-mounts/
- https://docs.docker.com/engine/storage/bind-mounts/
- https://docs.docker.com/engine/storage/volumes/

Documentation evidence review and local-link/whitespace checks accompany this update.
Task remains open; symlink relocation is the preferred implementation direction,
not a completed or fully qualified mount design.
