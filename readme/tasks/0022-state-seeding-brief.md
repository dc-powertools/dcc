# T-0022 Brief: Seed Declared State From The Image

## Identity And Source

- Task ID: T-0022
- Initial revision: r1
- Catalog: `readme/tasks/README.md`
- Accepted source: User instruction
- Source reference and date: Durable cache mounting review, 2026-07-21
- Parent or split task IDs: None
- Governing decision: `readme/decisions/0001-state-seeding-from-image.md`

## Goal

Data that a Feature `install.sh`, a `Dockerfile` layer, or an official `build` source
places at a declared `customizations.dcc.state` path is visible inside the container
instead of being silently masked by an empty bind mount, and it survives a wiped `.dcc`
directory without a rebuild.

## Background

State paths are attached as `type=bind` mounts. Unlike named Docker volumes, bind mounts
never copy image content into an empty host source. `CacheDir::prepare_state_mounts`
(`src/cache.rs`) materializes the host side **empty** before any container starts:
`StateKind::Directory` gets `create_dir_all`, `StateKind::File` gets a `create_new` empty
file.

So install-time content at a declared state path is present in the image but invisible at
runtime. The file case is worse than absence: a tool observes "exists, empty" and skips
its own initialization or fallback rather than failing cleanly.

The same masking applies during `dcc build`. `run_build_preparation` (`src/build.rs`) calls
`prepare_state_mounts` **before** `docker::start_detached`, so `onCreateCommand`,
`updateContentCommand`, and `postCreateCommand` also run against empty host directories
and cannot observe what install scripts produced. Lifecycle-hook output *is* persisted
correctly, so the current workaround is "regenerate the data in `onCreate`" — which only
works for data a hook can rebuild from scratch.

Two rejected alternatives are recorded in decision 0001 and must not be re-litigated
without new evidence: a **baked seed store** (an extra build stage staging tarballs into
the image) was rejected for image overhead, and **`mv` instead of `cp`** was rejected
because additive layers mean moving a path never reclaims the bytes of the layer that
created it, while additionally breaking standalone `docker run` use of the image.

## Scope

In scope:

- A hydration step that copies image content at each declared state path into the
  profile-local host state directory.
- A `dcc.seed` image label carrying the resolved seed manifest.
- A host-side ledger at `.dcc/<profile>.seed.json` recording per-entry `seed_digest` and
  `build_id`.
- Wiring hydration into `dcc build` **before** build-preparation hooks run.
- A guarded hydration check on `dcc start`, `run`, `exec`, and `attach` for the
  cloned-repo / wiped-`.dcc` case.
- Digest-based re-seed policy and warnings (see Re-Seed Policy).
- `--dry-run` reporting of planned seeding from the label, with no Docker invocation.
- Docker smoke coverage under the existing `#[ignore]` convention.
- README and `readme/project/architecture.md` documentation.

Out of scope:

- Critical-path guards; T-0021 owns those and should land first.
- The `/cache/runtime` container-writability exposure; T-0023 owns that.
- Any baked seed store or `mv`-based relocation (rejected in decision 0001).
- Changing the empty-file-state wart (see Known Limitations).

## Design To Complete

### Hydration mechanism

The image already holds the data at its natural path; only the mount hides it, and `dcc`
controls the mount. Hydrate with one short-lived container on the finished image with the
declared state mounts **not** applied and the host state root mounted at an unrelated
path:

```
docker run --rm -u root \
  --mount type=bind,src=<workspace>/.dcc/<profile>/state,dst=/dcc-seed \
  <image-tag> sh -c '<per-entry copy>'
```

Both sides are visible simultaneously, so no masking has to be worked around. The copy
must run **inside** the container (for example
`tar -C / -cf - <path> | tar -C /dcc-seed/<normalized> -xf -`) so uid, gid, mode, and
symlinks are preserved as the image intended. A host-side `docker cp` would instead land
every file owned by the invoking host user and break the container user's ability to write
its own cache afterwards.

`-u root` is required to read arbitrary image paths. The host state tree is
`.dcc/<profile>/state/<container path>` per `state_host_path`, so `/dcc-seed` maps onto
that root and each entry's destination is its normalized container path with the leading
`/` stripped.

### Path resolution

Reuse the existing resolver; do not reimplement path logic in shell. `run_build_preparation`
already establishes the pattern: `docker::inspect_image_env`, then conditional
`docker::probe_user_env`, then `resolve_runtime_state`. Hydration runs after the image
exists, so `${containerEnv:HOME}` resolves through that same path. Feature state merges
before project state, as it already does.

`uses_fast_path` returns false whenever `config.state` is non-empty, so any profile with
declared state already takes the Dockerfile path and the label is always available. There
is no fast-path gap to close.

### Seed manifest label

Emit a `dcc.seed` label alongside the existing `devcontainer.metadata` and `dcc.version`
labels, read back with the same `docker::inspect_image_label` idiom. Per entry: resolved
container path, kind, and a content digest. This is what lets `--dry-run` report planned
seeding without touching Docker, and what makes re-seed detection precise rather than
guesswork.

Open implementer decision: whether the digest is computed during the hydration container
run (cheap, same pass) or requires a separate inspection. Prefer the former. If a digest
cannot be computed for an entry, record its absence explicitly rather than a sentinel that
could collide with a real digest.

### Ledger

`.dcc/<profile>.seed.json`, recording per entry:

| Field | Purpose |
| --- | --- |
| `seed_digest` | Digest of the content `dcc` wrote at seed time |
| `build_id` | Image identity that produced the seed |

The ledger is **authoritative** for hydration decisions. Never infer intent from directory
emptiness: an empty seeded directory and an unseeded one must be distinguishable, and empty
file state is legitimate. This also removes the ordering fragility between
`prepare_state_mounts` and hydration — `prepare_state_mounts` may keep creating empty host
sources.

The ledger deliberately sits **outside** the `/cache` mount. A sentinel inside the profile
cache (`.dcc/<profile>/.dcc-seed`) would surface at `/cache/.dcc-seed` and be writable by
container-side code. Because profiles create directories, a sibling `<profile>.seed.json`
file cannot collide with a profile name. Do not mount the ledger into the container until
there is an actual in-container consumer.

### Build ordering

```
build image
  -> resolve state paths (inspect_image_env / probe_user_env / resolve_runtime_state)
  -> hydrate unseeded or safely-refreshable entries   [new]
  -> prepare_state_mounts
  -> start build-prep container
  -> onCreateCommand / updateContentCommand / postCreateCommand
```

Hydrating before build-prep is what fixes the hook blind spot: `onCreateCommand` can
finally observe install-time content.

Note that hydration needs its own container because build-prep runs *with* state mounts
attached and therefore cannot see the image content. Do not attempt to merge the two.

### Runtime guard

`dcc start`, `run`, `exec`, and `attach` must handle "cloned the repo, `.dcc` absent or
deleted, never rebuilt". Per user direction, content verification is too expensive for the
hot path: these commands compare the ledger's `build_id` against the image label only, and
**warn** on mismatch. They hydrate only entries with no ledger record at all.

### Re-Seed Policy

Per user direction:

- **`dcc build`**: re-digest the existing host state. If it matches the recorded
  `seed_digest`, the user has not modified it, so silently overwrite with the new seed.
  If it differs, the user has real data there — **warn and do not clobber**.
- **`dcc start` / `run` / `exec` / `attach`**: warn whenever the recorded `build_id` does
  not match the current image's, without re-digesting content.

Warnings must name the state path and both digests or build ids so the user can act. Also
name the escape hatch, which is a `--reseed-state` flag on `dcc build`. It overrides
the digest check so a differing host state is overwritten instead of preserved — the
direct answer to the mismatch warning. It is all-or-nothing across every declared state
path; a path-scoped form is deliberately deferred since re-seeding a path whose digest
already matches is a no-op, making the wider blast radius cosmetic rather than
destructive.

## Users And Workflows

| User/Actor | Workflow | Expected Change |
| --- | --- | --- |
| Developer using a Feature that installs a toolchain into a declared state path | `dcc build` then `dcc run` | Toolchain content is present instead of an empty directory |
| Developer with `{"path": ".../.npmrc", "type": "file"}` shipped by a Feature | Runs any container command | Sees the Feature's file content instead of an empty file |
| Developer who deletes `.dcc/` | `dcc run` | Warned; unseeded entries re-hydrate from the image without a rebuild |
| Developer who cloned the repo | `dcc build` | State is seeded on first build; no manual step |
| Developer who customized cached state | `dcc build` after a Feature upgrade | Warned that content differs; their data is preserved |
| Author of a build-prep hook | `onCreateCommand` reads a declared state path | Hook observes install-time content (previously empty) |
| Any user | `dcc build --dry-run --format json` | Report lists planned seeding; `docker_invoked: false` holds |

## Acceptance Criteria

- [ ] Directory state declared at a path where the image has content is populated with
      that content on first `dcc build`.
- [ ] File state declared at a path where the image has a file contains that file's
      content, not an empty file.
- [ ] Seeded content preserves uid, gid, and mode such that the container user can still
      write to a seeded directory (Linux; see Known Limitations for macOS).
- [ ] Build-prep hooks observe seeded content, proving hydration precedes them.
- [ ] Deleting `.dcc/` and running `dcc run` re-hydrates from the image with no rebuild.
- [ ] A second `dcc build` with unmodified state does not warn and does not duplicate work
      beyond the digest check.
- [ ] Host state modified by the user is **not** overwritten by `dcc build`; a warning
      names the path.
- [ ] `dcc start`/`run`/`exec`/`attach` warn on `build_id` mismatch and do **not**
      content-digest on the hot path.
- [ ] The ledger lives at `.dcc/<profile>.seed.json`, outside the `/cache` mount.
- [ ] `dcc build --dry-run --format json` reports planned seeding with
      `docker_invoked: false`.
- [ ] `dcc build --reseed-state --dry-run --format json` reports, per entry, whether the
      host state would be overwritten and why (digest differs from `seed_digest`, or no
      ledger record), without invoking Docker. The `docker_invoked: false` invariant holds
      because the ledger and host state are both host-side.
- [ ] A profile with no declared state performs no hydration container run at all.
- [ ] Ignored Docker smoke tests cover: directory seeding, file seeding, hook visibility,
      wiped-`.dcc` recovery, and the modified-state warning.
- [ ] README and architecture docs describe seeding, the ledger, and the warnings.

## Constraints

- Follow decision 0001: no baked seed store, no `mv`-based relocation.
- Reuse `inspect_image_env`, `probe_user_env`, and `resolve_runtime_state`; do not add a
  second state-path resolver.
- Copy inside the container to preserve ownership; never host-side `docker cp`.
- Hydration must be skipped entirely when there is no declared state — zero added cost for
  profiles that do not use it.
- `--dry-run` must not invoke Docker; seeding reporting comes from the label.
- `--dry-run --reseed-state` must not invoke Docker either; it reports the overwrite plan
  from the host-side ledger and host state, which are available without a container.
- Docker-dependent tests are `#[ignore]` and run in CI only
  (`readme/project/standards.md`).
- `anyhow::Result` with `.with_context`; no `unwrap`/`expect` outside `#[cfg(test)]`.
- Never log secrets; seeded paths may contain credentials, so log paths and digests, not
  content.

## Workflow Route Rationale

- Cataloged route and risk: See this task's catalog row.
- Why this route: New host-state artifact, a new Docker container invocation, changes to
  both build and runtime paths, and a user-visible warning policy.
- Why this risk gate: Hydration writes to the host cache and can overwrite developer data
  if the ledger logic is wrong; the no-clobber criterion is the critical one.
- Upstream artifacts required: `readme/decisions/0001-state-seeding-from-image.md`;
  T-0021 guards should land first so seeding cannot amplify a critical-path mistake.
- Escalation trigger: If hydration cost on large trees proves unacceptable, or if
  ownership preservation cannot be made to work on a supported platform, return to design
  and reconsider the baked-store option recorded in decision 0001.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Hydration overwrites developer-modified state | Data loss | Digest comparison against `seed_digest`; warn instead of clobber; write the no-clobber test first |
| Ownership not preserved, container user cannot write its own cache | Broken tooling inside the container | Copy inside the container as root via tar; assert writability in a Docker smoke test |
| macOS virtiofs maps bind-mount ownership to the host user | Weaker uid/gid fidelity than Linux | Document as a Known Limitation; add a Docker smoke assertion rather than assuming |
| A state path is a symlink in the image resolving outside itself (e.g. `/home/dev/.cache` -> `/etc`) | Hydration reads or writes an unintended tree | Hydration container refuses a state path that does not resolve within itself; this is the mitigation T-0021 defers here |
| Large state tree makes first build slow | Poor first-run experience | Report seeding progress and sizes in output; do not seed silently |
| Ledger and image label drift | Wrong warnings | Single writer for the ledger; label is the only source of `build_id` |
| Extra container run on every build | Slower builds | Skip entirely when the ledger shows all entries seeded and digests match |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| `tar` is present in supported base images | Medium | Verify in Docker smoke across Debian and Alpine; fall back to `cp -a` if absent |
| Reading arbitrary image paths requires root in the hydration container | High | `-u root` is explicit in the design |
| `uses_fast_path` already excludes any profile with declared state | High | Asserted by existing `build.rs` unit tests |
| A digest over seeded content is cheap enough for `dcc build` | Medium | Measure on a realistic toolchain-sized tree during implementation |
| Warning-only re-seed matches user expectation | High | Explicit user direction, 2026-07-21 |

## Verification Plan

- Automated checks: `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`,
  `cargo test`, `cargo build`.
- Unit tests: hydration planning from a manifest, ledger read/write round-trip, digest
  comparison branches (unseeded, matching, differing), `--reseed-state` overwrite
  planning without Docker, skip-when-no-state, and the hydration container argument
  construction (mirroring the existing `build_prep_container_args` test style).
- Ignored Docker smoke tests in `tests/docker_smoke.rs`: directory seeding, file seeding,
  build-prep hook visibility, wiped-`.dcc` recovery, modified-state no-clobber warning,
  `--reseed-state` clobber succeeding, and container-user writability of a seeded
  directory.
- Manual checks: `cargo run -- --dry-run --format json build` shows planned seeding and
  `docker_invoked: false`; `cargo run -- --reseed-state --dry-run --format json build`
  shows the per-entry overwrite plan with the same invariant.
- Documentation checks: README state section and architecture Cache Management /
  `dcc build` sections describe hydration, the ledger, and the warnings.
- Baseline evidence: write a Docker smoke test asserting seeded content **first** and
  observe it fail against current behavior (empty directory / empty file), establishing the
  masking defect before implementing.

## Known Limitations

- **Empty-file state remains misleading.** When no Feature ships the declared file, Docker
  still requires the bind source to exist, so `dcc` creates an empty file and a tool reads
  "exists, empty" rather than "absent". Seeding does not fix this; it is a separate wart
  and out of scope here.
- **macOS ownership fidelity** is weaker than Linux under Docker Desktop virtiofs.
- **Hydration cannot be cached in an image layer** by construction; it writes host state.
  The ledger, not a layer, is what prevents repeat work.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r1 | 2026-07-21 | User instruction | Initial intake | — | — |

## Done When

- Declared state is seeded from the image on build and recovers after a wiped `.dcc`.
- Developer-modified state is never silently overwritten.
- All acceptance criteria are satisfied with observed results, including the ignored Docker
  smoke tests executed in CI.
- Required checks pass and documentation matches the implementation.
