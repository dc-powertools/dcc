# 0010: Persistent data and image-owned directory exclusions

Date: 2026-11-16. Status: accepted implementation contract for T-0108 r6;
platform/catalog completion remains gated. Supersedes content-hash reconciliation in
0001 without deleting historical decision evidence.

Persist coherent directory roots. Add only literal relative directory exclusions,
with explicit stable installer-prepared image targets outside all durable roots.
Wire syntax: `type: "directory-with-exclusions"`,
`exclude: [{"path":"packages","target":"/opt/example/packages"}]`.
Older dcc silently ignores unknown object fields, so a new type discriminator is
required to fail closed. dcc 0.3.0 also refuses older built images/controllers.

Installers relocate in the installation layer and prepare writable targets; empty
scratch and populated software use identical container-lifetime semantics. dcc uses
serialized preparation containers to verify targets and create links through attached
state mounts before hooks. Symlinked roots/ancestors and conflicting metadata fail.
No general filter/glob engine, file-selection engine, per-container payload copy,
privileged mount, Docker backend change or new persistent storage backend.

Copy image defaults only when the whole host root is absent. Existing roots (including
empty ones) and old seed ledgers remain untouched. No automatic image defaults merge,
durable content hashing or tree traversal. Explicit hooks may initialize absent
settings. `--reseed-state` rejects before mutation. Existing isolated file binds remain
compatible with documented atomic-save limitations; new tools should persist parents.

Preserve displaced directories and stale links by rename into unique root-local
recovery directories. Never delete or merge old software into current image payload.
Host process locks serialize dcc initialization and migration; users must avoid manual
concurrent writes to migration paths. Failed initialization staging is recoverable.

Codex pilot retains mixed daemon preferences and .tmp marketplace data. Upstream
rejects symlinked app-server-control; retain it conservatively. Provision daemon payload
in image packages so upstream stale process recovery works after replacement. Pilot
qualification limits do not justify broad Feature rollout; macOS/Desktop is unavailable.
Release supporting dcc before publishing dependent Feature metadata. No publication
is authorized by this decision.
