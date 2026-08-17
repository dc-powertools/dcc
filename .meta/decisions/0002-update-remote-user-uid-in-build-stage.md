# 0002: Bake `updateRemoteUserUID` Into dcc's Generated Build Stage

Status: Accepted

Date: 2026-08-13

Owners:

- T-0026 implementer

Supersedes:

- None

Superseded by:

- None

## Context

`dcc` runs containers as `containerUser` (default `dev`, not root) and bind-mounts the
host workspace at `/workspace`, the profile cache at `/cache`, and declared state paths.
Docker bind mounts preserve host UIDs verbatim on Linux, so when the container user's UID
differs from the UID owning the host directory, the container user cannot write to it.
This is not hypothetical: on GitHub Actions runners the `runner` user is uid 1001 / gid
999 while a `useradd`-created `dev` is uid 1000, which broke
`seeded_directory_is_writable_by_container_user` once T-0024's exit-code fix stopped
masking it.

The devcontainer spec defines `updateRemoteUserUID` (default `true`) as the remedy: on
Linux, remap the container user's UID/GID to the local user's. The reference CLI
(`devcontainers/cli`, `scripts/updateUID.Dockerfile`) implements it as a **separate
derived image** (`<image>-uid`) built before every run from the resolved image, with
`ARG BASE_IMAGE/REMOTE_USER/NEW_UID/NEW_GID/IMAGE_USER`, a root `RUN` that `sed`-rewrites
`/etc/passwd`+`/etc/group` and `chown`s the user's home, then restores the original
`USER`.

## Decision

Fold the remap into `dcc`'s **own generated build stage** as a `RUN` immediately after
the existing user-creation step, rather than building a separate derived `<image>-uid`
image. The remap script ports the reference `updateUID.Dockerfile` `RUN` logic verbatim,
including every no-op condition (user not found, uid/gid already match, another user
occupies the target uid, a group occupies the target gid → keep the old gid and still
update the uid). `NEW_UID`/`NEW_GID` are passed as `docker build --build-arg` from the
host process uid/gid captured via `id -u`/`id -g` (`src/uid.rs::host_ids`).

`updateRemoteUserUID` is a recognized devcontainer config field on `RawConfig`
(defaulting to `true` on `DevcontainerConfig`), merged child-overrides-parent. The remap
is planned by `plan_uid_remap` and is a no-op when: the host is not Linux, the flag is
`false`, the user is `root`, the user is numeric, or the host uid/gid are unavailable.

## Rationale

`dcc` already owns a generated Dockerfile stage that creates `containerUser`, installs
features, and stamps the `dcc.version` / `devcontainer.metadata` / `dcc.seed` labels.
Adding the remap there avoids introducing a second image tag, so:

- Version-mismatch and seed-guard logic (`src/version.rs`, `src/seed.rs`) keep reading
  the single `image_tag` — no risk of misrouting label lookups to a derived tag.
- No derived `-uid` image proliferation and no cleanup problem on `dcc stop`/`dcc build`.
- The fast path (pull + retag, `containerUser: root`) is untouched: `uses_fast_path`
  requires root and the remap skips root, so the two never overlap (asserted by a unit
  test).

A separate derived image would have required threading a new tag through
`ContainerId::as_image_tag`, the version label lookup, the seed-label lookup, the
hydration container, the build-prep container, and the runtime launch — a wide change
with real risk of breaking the T-0022/T-0024 invariants users already depend on, for no
behavioral benefit given that `dcc` already owns the build stage.

## Seeding interaction

The build ordering stays:

```
build image (now includes the remap RUN)
  -> hydrate declared state from the image   [src/seed.rs, runs as root]
  -> prepare_state_mounts
  -> build-prep container (runs as containerUser, already remapped)
```

Hydration runs as root in a one-shot container with state mounts off and copies image
content with `tar`. Because the remap is baked into the image at **build time**, the
image's `/etc/passwd` already records the remapped uid before hydration runs.

Correction recorded 2026-08-17: Dockerfile layers can create and `chown` declared state
paths before the generated remap step. Those paths then keep the user's old numeric uid
even though `/etc/passwd` has been updated. Hydration therefore re-owns copied state to
the non-root `containerUser` after extraction; root profiles still preserve root
ownership. This keeps T-0022's hydration ordering while satisfying the writability
requirement.

## Consequences

- A non-root `containerUser` can write to bind-mounted host content on Linux regardless
  of the host user's uid, conforming to a default-on spec property.
- The remap is Linux-only; on macOS/Windows Docker Desktop translates uids in its VM and
  the remap is a no-op.
- `dcc build` now passes `--build-arg` values to the stdin-context `docker build`
  (`docker::build` accepts `build_args`).
- The reference's silent collision no-op is preserved (safety over convenience); the
  `RUN` echoes a recognizable `updateRemoteUserUID:` line for diagnosis.
- `remoteUser` as a distinct user from `containerUser` remains out of scope (deferred);
  the remap targets `containerUser`, which is `dcc`'s user of record.

## Alternatives Considered

- **Separate derived `<image>-uid` image** (reference approach): rejected for the
  threading risk and tag-proliferation cost described above.
- **`chown` host bind-mount content as the remedy**: rejected — destructive to user
  data and explicitly not what the spec prescribes; the reference chowns only the
  user's home folder inside the image.
- **Podman `--userns=keep-id`**: out of scope; Podman is not a common `dcc` target.

## References

- `.meta/tasks/0026-update-remote-user-uid-brief.md`
- `.meta/tasks/0026-r1-uid-remap-design.md`
- <https://containers.dev/implementors/json_reference/> (`updateRemoteUserUID`)
- `devcontainers/cli` `scripts/updateUID.Dockerfile`
