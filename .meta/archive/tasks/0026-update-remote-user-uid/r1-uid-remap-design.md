# T-0026 r1: `updateRemoteUserUID` Design

- Parent: T-0026
- Revision: r1
- Date: 2026-08-13

> Historical design note: T-0045 subsequently made the platform gate explicitly
> Linux-only. T-0054 then superseded that platform-scope outcome by adding macOS
> eligibility while preserving Windows as a no-op. The build-stage design below and
> its safety conditions remain current; its Linux-only statements describe the
> original T-0026 revision.

## Decision: fold the remap into dcc's own generated build stage

The reference CLI (`devcontainers/cli`, `scripts/updateUID.Dockerfile`) performs
the remap as a separate derived image (`<image>-uid`) built before every run.
`dcc` already owns a generated Dockerfile stage that creates `containerUser`,
installs features, and stamps the `dcc.version` / `devcontainer.metadata` /
`dcc.seed` labels. Adding the remap **there** — as a `RUN` step immediately
after user creation — avoids introducing a second image tag entirely, so:

- Version-mismatch and seed-guard logic keeps reading the single `image_tag`.
- No derived `-uid` image proliferation and no cleanup problem.
- The fast path (pull + retag, `containerUser: root`) is untouched: root is
  skipped, so the remap never runs there. The brief explicitly notes these two
  conditions never overlap and should be asserted.

### Why not a separate derived image (rejected)

Mirroring the reference's `<image>-uid` tag would require threading a new tag
through `ContainerId::as_image_tag`, the version label lookup, the seed-label
lookup, the hydration container, the build-prep container, and the runtime
launch — a wide change with real risk of breaking the T-0022/T-0024 invariants
users already depend on, for no behavioral benefit given that dcc already owns
the build stage.

## Remap step

Immediately after the existing idempotent user-creation `RUN`, when
`container_user != "root"` **and** `update_remote_user_uid` is true **and** the
host platform is Linux, emit a single `RUN` that ports the reference
`updateUID.Dockerfile` logic into inline `sh`:

1. `eval $(sed … /etc/passwd)` to read the target user's current `OLD_UID`,
   `OLD_GID`, and `HOME_FOLDER`.
2. `eval $(sed … /etc/passwd)` and `/etc/group` to detect an `EXISTING_USER` at
   `NEW_UID` and an `EXISTING_GROUP` at `NEW_GID`.
3. No-op (echo + exit 0 from the `RUN`) when: user not found, UID+GID already
   match, or another user already occupies `NEW_UID` (collision — refuse to
   stomp, matching the reference).
4. When a group already occupies `NEW_GID`, keep the old GID and still update
   the UID (reference behavior).
5. Otherwise `sed`-rewrite `/etc/passwd` and `/etc/group` and
   `chown -R NEW_UID:NEW_GID` the user's home folder.

`NEW_UID`/`NEW_GID` are passed as `docker build --build-arg` from the host
process uid/gid. The values are captured once at build time via
`id -u` / `id -g` (no new dependency), gated behind
`#[cfg(target_os = "linux")]`; non-Linux hosts report the no-op, while dry runs report
the platform decision without executing the build.

The remap `RUN` runs as root (the generated stage is root until the final image
inherits the base `USER`), which is exactly the reference's `USER root` block.

## Config property

Add `update_remote_user_uid: Option<bool>` to `RawConfig` (serde
`updateRemoteUserUID`, camelCase) and `update_remote_user_uid: bool` to
`DevcontainerConfig`, defaulting to `true`. Merge rule: child overwrites parent
(scalar override, same as `override_command`). It is a recognized devcontainer
property, so it is **not** collected into `extra`.

## Seeding interaction

The build ordering stays:

```
build image (now includes the remap RUN)
  -> hydrate declared state from the image   [seed.rs, runs as root]
  -> prepare_state_mounts
  -> build-prep container (runs as containerUser, already remapped)
```

Hydration runs as **root** in a one-shot container with state mounts off and
preserves image uids via `tar`. Because the remap now happens at **image build
time**, the image's `/etc/passwd` already records the remapped uid before
hydration runs, and any content the image places at a state path is owned by
the remapped uid. So seeded state is owned by the remapped user by construction
— no extra chown pass and no reordering of T-0022's hydration. (The brief's
"hydrate after the remap" question is answered: the remap is baked into the
image, so hydration already sees it.)

## Fast-path invariant

`uses_fast_path` requires `container_user == "root"`, and the remap skips
root, so the two never overlap. A unit test asserts `uses_fast_path` implies
the remap planning returns `Remap::None` for the same config.

## Collision no-op surfacing

The reference silently echoes "User with UID exists". `dcc` keeps the no-op
(safety over convenience) but the `RUN` echoes a recognizable line; on Linux a
follow-up debug/dry-run report can surface it. No new host-side parsing of the
build log is added in this revision.

## Tests (written first, watched fail)

Unit:

- `plan_uid_remap` no-op branches: root user, numeric user, already-matching
  uid/gid (same values), uid collision, gid collision (uid still updates),
  non-Linux (cfg gate), `updateRemoteUserUID: false`.
- `generate_dockerfile` emits the remap `RUN` only when non-root + enabled +
  Linux, with the correct `ARG`/`sed` shape; absent otherwise.
- `uses_fast_path` ⇒ remap is `None` (fast-path invariant).
- config: `updateRemoteUserUID` recognized, defaults to `true`, `false`
  respected, merged child-overrides-parent.

Docker smoke (`#[ignore]`, CI):

- `seeded_directory_is_writable_by_container_user` restored to the
  `/workspace/writable.txt` write + `fx.read_file("writable.txt")` assertion.
- non-root writes to `/workspace`, `/cache`, and a declared state path.
- a fast-path / root profile is unaffected.

## Out of scope (this revision)

- `remoteUser` as a distinct user from `containerUser` (brief permits deferring).
- Podman `--userns=keep-id`.
- Derived `-uid` image tagging/caching (rejected above).
- Removing derived `-uid` images on `dcc stop`/`dcc build` (no such images).
