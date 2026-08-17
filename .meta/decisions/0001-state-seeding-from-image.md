# 0001: Seed Declared State From The Image, Not From A Baked Seed Store

Status: Accepted

Date: 2026-07-21

Owners:

- T-0022 implementer

Supersedes:

- None

Superseded by:

- None

## Context

`customizations.dcc.state` paths are attached as `type=bind` mounts whose host sources
live under `.dcc/<profile>/state/...` (`src/cache.rs`). Bind mounts, unlike named Docker
volumes, never copy image content into an empty host source. `CacheDir::prepare_state_mounts`
eagerly materializes the host side *empty* before any container starts: directory state
gets `create_dir_all`, file state gets a `create_new` empty file.

Consequently, any data a Feature `install.sh`, a `Dockerfile` layer, or an official
`build` source wrote at a declared state path is present in the image but **invisible at
runtime**, silently replaced by an empty directory or an empty file. The file case is
worse than absence: a tool observes "exists, empty" and skips its own fallback or
initialization instead of failing cleanly.

The same masking applies during `dcc build`. `run_build_preparation` calls
`prepare_state_mounts` *before* `docker::start_detached` (`src/build.rs`), so
`onCreateCommand`, `updateContentCommand`, and `postCreateCommand` also run against the
empty host directories and cannot observe what the install scripts left behind. Lifecycle
hooks are correctly persisted, but they cannot see install-time content, so "regenerate it
in `onCreate`" only works for data a hook can rebuild from scratch.

A first-pass proposal was to inject a final build step that copies or moves the existing
data at each state path into a baked seed location inside the image, then hydrate the host
cache from it. Two constraints were then discovered that reshape the design:

1. `docker build` has no bind mount and no host filesystem access, so no build step can
   write to `.dcc/<profile>`. Hydration inherently writes host state and can never be
   captured in an image layer.
2. State paths may contain `${containerEnv:HOME}`, which `dcc` resolves only after the
   image exists, via `docker::inspect_image_env` plus a `docker::probe_user_env` run. A
   `RUN` step inside the same build cannot resolve it; as root, `$HOME` is `/root`, not
   the `dev` user's home.

`mv` was considered instead of `cp` to control image size. This does not work: layers are
additive, so moving or deleting a path in layer N never reclaims its bytes in layer N-1.
`mv` produces a whiteout for the original plus a full copy-up at the new path, costing
roughly the same as a copy while additionally making the image unusable outside `dcc`
(`docker run <image>` would find no data at the natural path).

## Decision

Do not bake a seed store into the image. The image already contains the data at its
natural path; the only thing hiding it is the mount, and `dcc` controls whether the mount
is applied.

Hydrate by running one short-lived container on the finished image with the declared state
mounts **not** applied and the host state root mounted at an unrelated path
(`/dcc-seed`). Both sides are then visible simultaneously, and the copy is performed
*inside* the container (`tar -C / -cf - <path> | tar -C /dcc-seed/<norm> -xf -`) so uid,
gid, mode, and symlinks are preserved as the image intended. A host-side `docker cp`
would instead land every file owned by the invoking host user.

Persistence of the seed source is satisfied by the image itself: if `.dcc` is wiped, the
same hydration container re-runs against the same image and reproduces the state.

What *is* baked is a small `dcc.seed` **label** carrying the resolved manifest (container
path, kind, digest). This extends the existing self-describing-image idiom already used
for `devcontainer.metadata` and `dcc.version` and lets `--dry-run` report what would be
seeded without invoking Docker.

Hydration decisions are driven by a host-side ledger at `.dcc/<profile>.seed.json`
recording, per entry, a `seed_digest` (digest of what `dcc` wrote) and a `build_id` (image
identity that produced it). The ledger is authoritative, so hydration never infers intent
from directory emptiness — an empty seeded directory and an unseeded one are
distinguishable, and empty file state remains legitimate.

The ledger deliberately sits *outside* the `/cache` mount. A sentinel inside the profile
cache directory (`.dcc/<profile>/.dcc-seed`) would appear at `/cache/.dcc-seed` inside the
container and be writable by container-side code. Because profiles create directories, a
sibling `<profile>.seed.json` file cannot collide with a profile name.

Seeding is on by default: silently empty state is the defect being corrected, so opt-in
would leave the broken behavior as the default experience.

## Options Considered

| Option | Pros | Cons | Notes |
| --- | --- | --- | --- |
| Baked seed store: `COPY` staged tarballs to `/usr/local/share/dcc/seed/` in an extra build stage, hydrate from there | Seed artifact pinned in image; staging cached as a layer; hydration folds into the existing build-prep container with no extra `docker run` | Duplicates seeded bytes in the image (~2x); needs a third build stage to resolve `${containerEnv:HOME}`; more moving parts | Rejected for image overhead once it was clear the image already holds the data |
| `mv` data into the seed location to control size | Intuitively avoids duplication | Does not reduce image size at all (additive layers); breaks standalone `docker run` usage; breaks any image whose state path is an OS directory | Rejected as ineffective |
| **Hydrate from the image at its natural path via an unmasked container (chosen)** | Zero image overhead; image stays standalone-usable; survives a wiped `.dcc`; reuses the existing `inspect_image_env` + `probe_user_env` + `resolve_runtime_state` resolver | Costs one extra short `docker run` on the seeding path | Chosen |
| Do nothing; require lifecycle hooks to regenerate state | No new machinery | Silent data masking persists; hooks cannot see install-time content; empty-file state actively misleads tools | Rejected |

## Consequences

Positive:

- No image size increase and no `mv`-induced image breakage.
- Feature- and `Dockerfile`-installed content at declared state paths becomes visible at
  runtime instead of being silently masked.
- Hydrating before build-prep means `onCreateCommand` and the other build-prep hooks can
  finally observe install-time content.
- One canonical state-path resolver is reused; no second shell-side reimplementation that
  could diverge.
- `--dry-run` can report planned seeding from the label without touching Docker.

Negative:

- One extra short-lived `docker run` whenever hydration is required.
- Hydration is inherently a host-state write and can never be cached in the image; the
  ledger, not a layer, is what prevents repeat work.

Neutral or follow-up:

- `uses_fast_path` already returns false when `config.state` is non-empty, so any profile
  with declared state takes the Dockerfile path and the manifest label is always
  available. No fast-path gap to close.
- On macOS with Docker Desktop, virtiofs maps bind-mount ownership to the host user, so
  in-container uid/gid preservation is weaker than on Linux. Seeded content works but does
  not round-trip ownership identically; this needs a Docker smoke assertion rather than an
  assumption.
- Empty-file state remains misleading when no Feature ships the file, because Docker
  requires the bind source to exist. Seeding does not fix this; it is a separate wart.

## Confidence

Confidence: High

Why: The masking behavior is directly readable in `src/cache.rs` and `src/build.rs`, and
the additive-layer property that defeats `mv` is a documented Docker/OCI invariant rather
than an empirical guess. The residual uncertainty is operational (macOS ownership
mapping, hydration cost on large trees), not architectural.

## Review Trigger

Revisit this decision when:

- Hydration cost on large state trees becomes a user-visible complaint, which would
  reopen the baked-store option as a caching optimization.
- `dcc` gains a Docker-volume-backed state mode, where the runtime would copy image
  content into a fresh volume automatically and hydration would be unnecessary.
- A supported use case requires the image to be usable outside `dcc` with state paths
  already relocated.

## Sources

- `src/cache.rs` (`prepare_state_mounts`, `plan_state_mounts`, `to_mount_arg`)
- `src/build.rs` (`run_build_preparation`, `build_prep_container_args`, `uses_fast_path`)
- `src/exec.rs` (runtime mount construction)
- `src/config/resolve.rs` (`normalize_state_path`, `resolve_state_entries_container_env`)
- User design review, 2026-07-21
