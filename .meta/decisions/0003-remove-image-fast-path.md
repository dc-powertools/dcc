# 0003: Remove The Image Fast Path

Status: Accepted

Date: 2026-08-13

Owners:

- T-0027 implementer

Supersedes:

- None

Superseded by:

- None

## Context

`uses_fast_path` (`src/build.rs`) short-circuited `dcc build` for an image-only
profile with `containerUser: root` and no features, containerEnv, forwardPorts,
build-prep hooks, or declared state: it ran `docker pull` + `docker tag` and skipped
`build_dcc_stage` entirely. No `dcc.version` label was stamped, no generated assets
reached the image, and no lockfile was written.

This shortcut was a standing tax on several features:

- T-0024 could not bake the supervisor into the image; it fell back to the read-only
  `rt` bind mount.
- T-0022 had to ensure `uses_fast_path` returned false whenever state was declared.
- T-0026 needed a fast-path-implies-no-remap invariant plus a guard test.
- T-0025 could not install packages the supervisor might need (e.g. `inotify-tools`),
  which contributed to choosing the dependency-free FIFO handshake.

`version.rs` also branched on the fast-path flag to decide whether a missing
`dcc.version` label meant "expected, the fast path never stamps" or "stale image".

A design investigation (T-0027 brief) established that for a fast-path config the dcc
build stage generates a two-line Dockerfile — `FROM <image>` plus `LABEL dcc.version` —
because every other section is already gated behind an `is_empty()` or non-root check.
This is asserted today by the existing `dockerfile_root_user_skips_creation` test. So
removing the fast path does not make these builds meaningfully more expensive: it adds
one cached `LABEL` layer over an image Docker already has.

## Decision

Remove the image fast path. Every `dcc build` goes through `build_base_image` +
`build_dcc_stage`, so every dcc-built image carries a `dcc.version` stamp.

Three consequences were reviewed and decided:

### C1: base-image freshness moves under `--update`

The fast path pulled the image on every build. `docker build` does not pass `--pull`,
so a `FROM <image>` over a local image would reuse the stale copy rather than
re-resolving the tag upstream.

**Decision:** pass `--pull` to `docker build` if and only if `--update` was given.
`--update` already means "re-resolve my inputs" for feature digests (it discards
`locked_digests`); extending it to the base image applies the same promise to the other
input class. The default build stays fast and offline-friendly, and freshness becomes
explicit and discoverable rather than an invisible per-build registry round trip.

### C2: suppress the empty lockfile write

`build_dcc_stage` ends with `write_lockfile`, which the fast path never reached, so
previously-fast-path profiles would gain a `devcontainer.lock` with an empty feature
list next to `devcontainer.json` — a new file in a directory users often version-control.

**Decision:** skip the write when `lock_entries` is empty and no lockfile already exists.
If a lockfile is already present it is still rewritten, so a profile that drops its last
feature correctly ends up with an empty feature list rather than a stale one.

### C3: delete `docker::pull` and `docker::tag`

The fast-path branch was the only caller of both. Under C1 the freshness behavior is a
`--pull` flag on `docker build`, not a call to `docker::pull`.

**Decision:** delete both functions and their unit tests.

### Side effect: `--no-cache` is now honored

The fast path explicitly discarded `--no-cache` (`let _ = opts.no_cache;`). After
removal, `--no-cache` reaches `docker build` for all profiles. This is a strict
improvement.

## Consequences

- `uses_fast_path` and its branch are gone; `version_warning` loses its
  `current_uses_fast_path` parameter and warns on any missing `dcc.version` label.
- `docker::pull` and `docker::tag` are deleted.
- `stop.rs`'s local `current_uses_fast_path` helper is deleted (the only caller of the
  parameter that could pass `None`).
- The vacuous `fast_path_config_implies_no_remap` test in `uid.rs` is deleted (a weaker
  duplicate of the existing `plan_skips_root_user`).
- `--update` now also re-pulls the base image; its help text says so.
- The `rt` bind mount and supervisor delivery model are **unchanged** — removing the
  fast path does not resurrect baking the supervisor into the image. That question is
  tracked separately as T-0028, which depends on T-0027 landing.

## Alternatives Considered

- **Pass `--pull` on every build** (C1 option a): most faithful to the old behavior, but
  costs a registry round trip per build and makes the default online-only. Rejected.
- **Accept the staleness and document it** (C1 option c): cheapest, but silently drops a
  freshness guarantee users had. Rejected.
- **Always write the lockfile** (C2 alternative): uniform, but the content is an empty
  list and the uniformity argument is weak. Rejected.
- **Keep `docker::pull`/`tag` for future use** (C3 alternative): keeping unused wrappers
  is the kind of dead surface this task exists to remove. Rejected.
