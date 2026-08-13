# T-0026 Brief: Implement `updateRemoteUserUID` For Bind-Mount Permission Correctness

## Identity And Source

- Task ID: T-0026
- Initial revision: r1
- Catalog: `readme/tasks/README.md`
- Accepted source: User direction after reviewing the T-0024 smoke-test permission failure
- Source reference and date: UID-alignment review, 2026-08-13
- Parent or split task IDs: None

## Goal

`dcc` implements the devcontainer specification's `updateRemoteUserUID` property so a
non-root `containerUser` can read and write bind-mounted host content (the workspace, the
cache, and declared state) without depending on the host user's UID happening to match the
container user's UID. `seeded_directory_is_writable_by_container_user` is restored to its
original workspace-writing form and passes.

## Background

`dcc` runs containers as `containerUser`, which **defaults to `dev`** (not root) —
see `src/config/resolve.rs`. It bind-mounts the host workspace at `/workspace`, the
profile cache at `/cache`, and declared state paths, then runs commands as that user.

Docker bind mounts preserve host UIDs verbatim; there is no translation layer on Linux. So
when the container user's UID differs from the UID owning the host directory, the container
user cannot write to it. This is not hypothetical:

- On GitHub Actions runners the `runner` user is uid 1001 / gid 999, while a
  `useradd`-created `dev` gets uid 1000. This exact mismatch caused
  `seeded_directory_is_writable_by_container_user` to fail with
  `cannot create /workspace/writable.txt: Permission denied` once T-0024's exit-code fix
  stopped masking it.
- The same class of failure is widely reported in the ecosystem, e.g.
  `devcontainers/images#723` (GitHub Actions runner uid 1001 / gid 999 breaking
  devcontainers) and `devcontainers/images#1056` / `#1542` (Ubuntu 24.04 shipping an
  `ubuntu` user at uid 1000, pushing `vscode` to 1001).

The devcontainer specification already defines the remedy, and defines it as **on by
default**. From the official reference
(<https://containers.dev/implementors/json_reference/>):

> `updateRemoteUserUID` | boolean | On Linux, if `containerUser` or `remoteUser` is
> specified, the user's UID/GID will be updated to match the local user's UID/GID to avoid
> permission problems with bind mounts. Defaults to `true`.

`dcc` does not implement this property at all. It is therefore non-conformant on a
default-on spec property whose entire stated purpose is preventing the failure above.
`remoteUser` is likewise unimplemented (warned in default mode, rejected under `--strict`).

### Correction to a prior change

T-0024 modified `seeded_directory_is_writable_by_container_user` to write inside
`/seeded-dir` and read back from the host state directory, instead of writing to
`/workspace`. That was wrong and must be reverted. The test's purpose is to verify the
seeded directory is writable **in its standard location** — that is, as a bind-mounted
state path with the workspace write that a real user performs. Relocating the write turned
it into an assertion about seed files in an arbitrary location and silently deleted the
only smoke coverage of a non-root user writing to the workspace. The workaround comment
added there ("the workspace bind mount is owned by the host runner user whose uid may
differ") described a `dcc` defect as if it were an environmental given.

## Design

### Approach: follow the reference implementation

The reference CLI (`devcontainers/cli`, `scripts/updateUID.Dockerfile`) performs the remap
as a **pre-run image derivation**, not at container start:

1. Read the host user's uid/gid (`getuid()` / `getgid()`).
2. Build a derived image tagged `<image>-uid` from the resolved image, with build args
   `BASE_IMAGE`, `REMOTE_USER`, `NEW_UID`, `NEW_GID`, `IMAGE_USER`.
3. In that layer, as root: `sed`-rewrite the target user's UID/GID in `/etc/passwd` and
   `/etc/group`, then `chown -R $NEW_UID:$NEW_GID` the user's home folder, then restore
   the original `USER`.
4. Run the container from the derived image.

It deliberately **no-ops** in these cases:

- the target user is `root`;
- the target user is already a numeric UID;
- the UID and GID already match the host's;
- **another user already occupies the target UID** (collision — it refuses to stomp on the
  existing user; this is the Ubuntu 24.04 failure mode);
- a group already occupies the target GID (keeps the old GID, still updates the UID);
- the host platform is not Linux.

`dcc` should mirror this behavior, including the no-op conditions, because they are
load-bearing safety properties rather than incidental details.

### Platform scope

Linux only, matching the spec and the reference implementation. On macOS and Windows,
Docker Desktop performs UID translation inside its VM, so the remap is unnecessary; the
spec explicitly permits skipping when the engine translates automatically. Implement as a
no-op on non-Linux hosts.

Podman's `--userns=keep-id` (a runtime kernel-level remap the reference CLI auto-applies)
is **out of scope** — Podman is not a common target for `dcc`.

### Interaction with the fast path

`uses_fast_path` (`src/build.rs`) already requires `container_user == "root"`, and the
remap skips root entirely, so the two never overlap. A fast-path profile needs no remap by
construction. This removes the fast-path hazard that complicated T-0024, but the
implementer should assert this invariant rather than assume it, since a future change to
either condition would silently reintroduce the gap.

### Remaining implementer decisions

- Whether to add `updateRemoteUserUID` as a recognized config property (defaulting to
  `true`) and whether to implement `remoteUser` alongside it, or to drive the remap solely
  from `containerUser`. The spec's precedence is `remoteUser` → `runArgs --user` → image
  `USER`; `dcc` has no `remoteUser`, so `containerUser` is the natural target.
- Where the derived image fits relative to the existing image-tag scheme
  (`ContainerId::as_image_tag`), the `dcc` version label, and the `dcc.seed` label, so
  version-mismatch and seed-guard logic keep working against the right tag.
- Whether the derived image is rebuilt on every run or cached and invalidated on host
  uid/gid change.
- How this composes with state seeding: the hydration container runs as root and tar
  preserves image uids, so seeded content carries the image's original uid. If the user's
  uid is remapped afterwards, previously seeded state may no longer be owned by the
  remapped user. Determine whether hydration must run after the remap, or whether seeded
  paths need chowning to the remapped uid.
- Whether `dcc stop`/`dcc build` cleanup should remove derived `-uid` images.
- What to do when the collision no-op triggers: silently proceed (matching the reference)
  or emit a warning naming the occupying user, which would have made the Ubuntu 24.04 class
  of failure far easier to diagnose.

## Scope

In scope:

- Implement `updateRemoteUserUID` semantics: derived-image UID/GID remap before container
  creation, with all reference no-op conditions.
- Recognize the `updateRemoteUserUID` config property, defaulting to `true`.
- Apply to the runtime launch path (`src/exec.rs`) and the build-preparation container
  (`src/build.rs`), consistent with how T-0024 treated both launch sites.
- Resolve the interaction with state seeding ownership.
- **Revert `seeded_directory_is_writable_by_container_user` to its original form**: the
  `write` command is
  `printf writable > /seeded-dir/after && cat /seeded-dir/after > /workspace/writable.txt`,
  asserted via `fx.read_file("writable.txt")`, with the workaround comment removed.
- Tests: unit coverage for the no-op conditions and remap argument construction; ignored
  Docker smoke coverage for a non-root user writing to the workspace, the cache, and a
  declared state path.
- Update README and `readme/project/architecture.md`, including the `remoteUser` /
  `containerUser` documentation rows.

Out of scope:

- Podman `--userns=keep-id` and any container-engine detection.
- Implementing `remoteUser` as a distinct user from `containerUser` (may be considered if
  the implementer finds it necessary for spec precedence; otherwise a separate task).
- Docker daemon `userns-remap` configuration.
- The T-0022 root-owned-seeded-state removal friction (`.dcc` wipe requiring root),
  except where the seeding-ownership interaction above forces a decision.
- T-0025 startup/readiness sequencing.

## Users And Workflows

| User/Actor | Workflow | Expected Change |
| --- | --- | --- |
| Developer on Linux with a non-root `containerUser` | `dcc exec`, `dcc run` writing to `/workspace` | Writes succeed regardless of host uid; previously failed when uids differed |
| Developer whose host uid is not 1000 | Any bind-mount write | Works without manual `chown` or switching to root |
| CI (GitHub Actions, uid 1001 / gid 999) | Docker smoke tests with `containerUser: dev` | Workspace and state writes succeed |
| Developer on macOS/Windows | Any | No behavior change; remap is a no-op |
| Developer using an image where the target uid is taken (e.g. Ubuntu 24.04) | Container start | Remap safely no-ops rather than corrupting the image's user table |
| Developer using the fast path (`containerUser: root`) | Any | No behavior change; root is skipped |

## Acceptance Criteria

- [ ] `updateRemoteUserUID` is a recognized config property defaulting to `true`.
- [ ] On Linux with a non-root `containerUser`, the container user's UID/GID is remapped
      to the host user's before the container is created.
- [ ] The remap no-ops when: the user is `root`; the user is numeric; UID and GID already
      match; another user occupies the target UID; the host is not Linux.
- [ ] When a group already occupies the target GID, the GID is left unchanged and the UID
      is still updated (reference behavior).
- [ ] `seeded_directory_is_writable_by_container_user` is restored to writing
      `/workspace/writable.txt` and asserting via `fx.read_file("writable.txt")`, and it
      passes in CI.
- [ ] A non-root `containerUser` can write to `/workspace`, `/cache`, and declared state
      paths, covered by ignored Docker smoke tests.
- [ ] Seeded state remains writable by the container user after the remap.
- [ ] The build-preparation container and the runtime container behave consistently.
- [ ] Fast-path profiles (`containerUser: root`) are unaffected, asserted by a test.
- [ ] Existing durable/one-shot reuse, `--keep`, and all `dcc stop` variants still work.
- [ ] README and `readme/project/architecture.md` document `updateRemoteUserUID` and
      correct the `remoteUser`/`containerUser` rows.
- [ ] Required checks pass: `cargo fmt --check`, `cargo check`,
      `cargo clippy -- -D warnings`, `cargo test`, `cargo build`.

## Constraints

- Follow the reference implementation's no-op conditions exactly; they prevent corrupting
  images whose user table already uses the target UID.
- Do not `chown` host bind-mount content as the remedy. The reference chowns only the
  user's home folder inside the image; rewriting host ownership is destructive to user data
  and is explicitly not what the spec prescribes.
- The remap must not run for `root`, which would be both pointless and destructive.
- Docker-dependent tests are `#[ignore]` and run in CI only
  (`readme/project/standards.md`).
- `anyhow::Result` with `.with_context`; no `unwrap`/`expect` outside `#[cfg(test)]`.
- Keep `--dry-run` Docker-free: the remap must be reported, not executed, under dry run.

## Workflow Route Rationale

- Cataloged route and risk: See this task's catalog row.
- Why this route: Adds a spec-conformance behavior that mutates the image used for every
  runtime command, affecting all bind-mount access for non-root users.
- Why this risk gate: An incorrect remap can corrupt an image's user table or silently
  break file access for every command; the no-op conditions are safety-critical and need
  live Docker evidence.
- Upstream artifacts required: devcontainer spec reference
  (<https://containers.dev/implementors/json_reference/>); reference implementation
  `devcontainers/cli` `scripts/updateUID.Dockerfile`.
- Escalation trigger: If resolving the seeding-ownership interaction requires changing when
  or how state is hydrated, escalate before implementing — that alters T-0022 behavior that
  users already depend on.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Remap corrupts the image user table on UID collision | Broken container, unusable image | Implement the collision no-op; test against an image whose target uid is already taken |
| Derived `-uid` images proliferate | Disk bloat, confusing `docker images` output | Decide caching/invalidation and cleanup explicitly |
| Derived image breaks version-label or seed-label lookups | Spurious rebuild warnings or seed guard misfires | Keep labels on the derived image; test `dcc build` → `dcc run` → version warning path |
| Seeded state ends up owned by the pre-remap uid | Seeded content unreadable/unwritable by the container user | Resolve hydration ordering; smoke-test seeded write after remap |
| Remap runs on macOS/Windows where it is meaningless | Wasted build, possible breakage | Gate on Linux; test the no-op path |
| Fast-path and remap conditions drift apart in future changes | Silent reintroduction of the permission bug | Assert the root-only fast-path invariant in a test |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| `containerUser` is the correct remap target given `dcc` has no `remoteUser` | High | Spec precedence is `remoteUser` → `--user` → image `USER`; `dcc` sets the user via `containerUser` |
| The fast path never needs the remap | High | `uses_fast_path` requires `container_user == "root"`, and root is skipped; assert in a test |
| Host uid/gid are obtainable without a new dependency | High | `std::os::unix::fs::MetadataExt` on a host-created path, or `id -u`/`id -g`; T-0024 used the metadata approach |
| macOS/Windows need no remap | High | Spec permits skipping when the engine translates; Docker Desktop does so in its VM |

## Verification Plan

- Automated checks: `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`,
  `cargo test`, `cargo build`.
- Unit tests: each no-op condition (root, numeric, already-matching, UID collision, GID
  collision, non-Linux); derived-image build argument construction; config default of
  `true` and explicit `false`.
- Ignored Docker smoke tests: restored
  `seeded_directory_is_writable_by_container_user`; non-root writes to `/workspace`,
  `/cache`, and a declared state path; an image whose target uid is already occupied
  (no-op, container still usable); a fast-path/root profile unaffected; durable and
  one-shot reuse after remap.
- Manual checks: `dcc --debug exec` shows the remap decision and resulting image tag;
  `id -u` inside the container matches the host uid.
- Documentation checks: README and architecture describe the property, its default, its
  Linux-only scope, and the no-op conditions.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r1 | 2026-08-13 | User direction | Initial intake | — | — |

## Done When

- A non-root `containerUser` can write to bind-mounted host content on Linux regardless of
  the host user's uid.
- The remap safely no-ops in every reference-defined condition.
- `seeded_directory_is_writable_by_container_user` is restored to its original workspace
  write and passes.
- Required checks pass and documentation matches the implementation.
