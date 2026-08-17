# T-0027 Brief: Remove The Image Fast Path

## Identity And Source

- Task ID: T-0027
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User direction during the T-0025 design review
- Source reference and date: T-0025 r3 design pass, 2026-08-13
- Parent or split task IDs: Split out of T-0025 (D0). Not a T-0025 prerequisite.

## Goal

Every container `dcc` creates is built through the dcc build stage, so every image
carries dcc's own machinery and version stamp. `uses_fast_path` and its pull-and-tag
branch are deleted, along with the special cases it forced into `version.rs`, `run.rs`,
`stop.rs`, `exec.rs`, and `uid.rs`.

## Background

`uses_fast_path` (`src/build.rs`) short-circuits `dcc build` for a narrow class of
profile: an `image` source, no `build`, no features, no `containerEnv`, no
`forwardPorts`, no build-prep hooks, no declared state, **and** `containerUser: root`.
For those it runs `docker pull` + `docker tag` and skips `build_dcc_stage` entirely, so
nothing dcc generates reaches the image.

That shortcut has been a standing tax on every feature built since:

| Task | Tax paid |
| --- | --- |
| T-0024 | The supervisor could not be baked into the image; forced the read-only `rt` bind-mount design |
| T-0022 | `uses_fast_path` must return false whenever `state` is declared |
| T-0026 | Needed a fast-path-implies-no-remap invariant plus a guard test |
| T-0025 | Blocked installing any package the supervisor might need (e.g. `inotify-tools`) |

It is also load-bearing in `version.rs`, which uses it to decide whether a missing
`dcc.version` label means "expected, the fast path never stamps" or "stale image".

The decisive finding from the design investigation: for a fast-path config the dcc build
stage generates a **two-line Dockerfile**.

```dockerfile
FROM debian:bookworm-slim
LABEL dcc.version='<version>'
```

Every other Dockerfile section — user creation, uid remap, feature install, `nc`
install, generated assets, `containerEnv` — is already gated behind a non-empty or
non-root check and is omitted. This is asserted today by the existing
`dockerfile_root_user_skips_creation` test in `src/features/context.rs`. So the removal
does not make these builds meaningfully more expensive; it adds one cached `LABEL`
layer over an image Docker already has.

## Design

### Delete the branch, keep everything else

`build()` reduces to `if opts.refresh_only { … } else { build_base_image +
build_dcc_stage }`. `build_base_image` already returns `config.image` unchanged when
`config.build` is `None`, so an image-source profile flows through without an extra
build of its own; only the dcc stage runs.

The investigation confirmed no code path in `build_base_image`, `build_dcc_stage`,
`resolve_features`, `topological_sort`, or `run_build_preparation` breaks on empty
features / empty env / root user. Every one is guarded by an `is_empty()` /
`!= "root"` / `Option` check. There are no panics, unwraps on empty, or index errors on
this path.

### `version.rs` loses the parameter

With every image stamped, a missing `dcc.version` label is unambiguously a stale image.
`warn_if_image_version_mismatch` and `warn_if_image_version_mismatch_best_effort` drop
`current_uses_fast_path: Option<bool>`, and `version_warning` collapses three `None`
arms into one:

```rust
match image_version {
    Some(version) if version == current => None,
    Some(version) => Some(/* explicit mismatch, unchanged */),
    None => Some(/* "does not record the dcc version"; reuse existing text */),
}
```

The existing `Some(false)` message text is reused verbatim, so the warning a user sees
is unchanged in wording.

### Three settled consequences

These are the reason this task is `Initiative / Medium` rather than a quick change.
All three were reviewed and accepted on 2026-08-13; they are decisions, not options.

**C1 — base-image freshness moves under `--update`. (Decided.)** The fast path
explicitly pulled the image on every build. `docker build` does **not** pass `--pull`,
so a `FROM <image>` over an already-present local image would reuse the local copy
rather than re-resolving the tag upstream, and a user whose upstream tag moved would
silently keep a stale image.

**Decision: pass `--pull` to `docker build` if and only if `--update` was given.**
`--update` already means "re-resolve my inputs" for Feature digests (it discards
`locked_digests`); extending it to the base image is the same promise applied to the
other input class. The default build stays fast and offline-friendly, and freshness
becomes explicit and discoverable rather than an invisible per-build round trip.

Implementation: add `pull: bool` to `DockerBuildOptions`, emit `--pull` in
`docker::build_args` when set, and plumb `opts.update` through `build_dcc_stage` to the
`docker::build` call. Update the `--update` help text to say it also re-pulls the base
image, and note it in the README build section. `build_args_for_stdin_context_are_deterministic`
in `src/docker.rs` needs the new field.

**C2 — suppress the empty lockfile write. (Decided.)** `build_dcc_stage` ends with
`write_lockfile`, which the fast path never reached, so previously-fast-path profiles
would gain a `devcontainer.lock` containing `{"dccVersion": "…", "features": []}` next
to `devcontainer.json` — a new file appearing in a directory users often have in version
control.

**Decision: skip the write when `lock_entries` is empty and no lockfile already
exists.** If a lockfile is already present it is still rewritten, so a profile that
drops its last feature correctly ends up with an empty feature list rather than a stale
one. No new file appears in anyone's repository. The uniformity argument for always
writing is weak when the content is an empty list.

**C3 — delete `docker::pull` and `docker::tag`. (Decided.)** Confirmed by grep that the
fast-path branch is their only caller. Under C1 the freshness behavior is a `--pull`
*flag on `docker build`*, not a call to `docker::pull`, so both functions become dead
once the branch is gone.

**Decision: delete both, along with their unit tests.** Keeping an unused Docker wrapper
"just in case" is the kind of dead surface this task exists to remove.

### `--no-cache` starts being honored

The fast path explicitly discarded it (`let _ = opts.no_cache;`). After removal,
`--no-cache` reaches `docker build` for these profiles. This is a strict improvement and
needs no decision, but should be noted in the changelog.

## Scope

In scope:

- Delete `uses_fast_path` and the pull-and-tag branch in `src/build.rs`, plus the
  `dcc debug: fast path` line and the 7 unit tests that exercise it.
- Drop the `current_uses_fast_path` parameter from both `version.rs` entry points and
  collapse `version_warning`'s `None` arms; update/delete the 5 affected tests.
- Remove fast-path plumbing at the three call sites: `src/exec.rs` (2 lines),
  `src/run.rs` (2 lines), `src/stop.rs` (2 lines plus the whole local
  `current_uses_fast_path` helper).
- Delete the now-vacuous `fast_path_config_implies_no_remap` test in `src/uid.rs` (a
  strictly weaker duplicate of the existing `plan_skips_root_user`).
- Implement C1 (`--pull` under `--update`), C2 (skip the empty lockfile write), and C3
  (delete `docker::pull`/`tag`) as decided above.
- Update `README.md` and `.meta/project/architecture.md` fast-path documentation
  (architecture.md ~lines 478–483, 500–506, 762–765, 928–930; README.md ~lines 336,
  365–370 — these were deliberately left untouched by T-0025).
- A decision record for the behavior change.

Out of scope:

- Any change to the supervisor, the `rt` bind mount, or the T-0025 startup handshake.
  Removing the fast path does **not** resurrect baking the supervisor into the image;
  the `rt` mount stays, because it keeps supervisor scripts read-only from inside the
  container and lets a `dcc` upgrade fix the supervisor without an image rebuild.
- Feature resolution, state seeding, or uid remap logic.

## Users And Workflows

| User/Actor | Workflow | Expected Change |
| --- | --- | --- |
| Developer with a plain `{"image": …, "containerUser": "root"}` profile | `dcc build` | One extra cached `docker build` of a 2-line Dockerfile instead of pull+tag; image gains a `dcc.version` label |
| Same developer | `dcc build` after the upstream tag moves | No re-pull by default; `dcc build --update` re-pulls the base image |
| Same developer | `dcc build --no-cache` | Now actually honored (previously silently ignored) |
| Any developer | `dcc exec` / `run` / `stop` on an image with no version stamp | Now warns "does not record the dcc version"; previously silent for fast-path profiles |
| Maintainer | Adding a feature that needs image-side support | No longer has to ask "does this work on the fast path?" |

## Acceptance Criteria

- [ ] `uses_fast_path` no longer exists; `grep -rn uses_fast_path src/ tests/` is empty.
- [ ] `dcc build` on `{"image": "debian:bookworm-slim", "containerUser": "root"}`
      succeeds and produces an image carrying a `dcc.version` label matching the current
      dcc version.
- [ ] `version_warning` takes no fast-path parameter and warns on a missing label.
- [ ] C1: `docker build` receives `--pull` when `--update` is given, and does not
      otherwise; `--update` help text and README say so.
- [ ] C2: `dcc build` on a feature-less profile creates no `devcontainer.lock`; a
      profile that already has one still gets it rewritten (empty feature list) when its
      last feature is removed.
- [ ] C3: `docker::pull` and `docker::tag` are deleted along with their unit tests.
- [ ] `--no-cache` reaches `docker build` for a previously-fast-path profile.
- [ ] `stop.rs`'s local `current_uses_fast_path` helper is gone and no import is
      orphaned (`CacheDir`/`config` remain used by the dry-run branch).
- [ ] README and architecture fast-path documentation updated; a decision record exists.
- [ ] Required checks pass: `cargo fmt --check`, `cargo check`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build`.

## Constraints

- `anyhow::Result` with `.with_context`; no `unwrap`/`expect` outside `#[cfg(test)]`.
- Docker-dependent tests are `#[ignore]` and run in CI only.
- The `rt` bind mount and supervisor delivery model are unchanged (see Out of scope).

## Workflow Route Rationale

- Cataloged route and risk: Initiative / Medium.
- Why this route: Touches the build entry path and six files, and carries three
  user-visible behavior changes (C1–C3) that need explicit decisions rather than a
  mechanical deletion.
- Why this risk gate: An error breaks `dcc build` for the simplest possible profile —
  the one most likely to be someone's first experience of dcc. Needs live Docker
  evidence, not unit tests alone. Not High: the change is subtractive, the affected
  code paths are already proven by every non-fast-path profile, and the generated
  Dockerfile for the affected class is two lines.
- Upstream artifacts required: `.meta/tasks/0025-r1-startup-handshake-design.md` (D0).
- Escalation trigger: If `--pull` under `--update` (C1) proves insufficient — for
  example if a user-visible workflow depends on every `dcc build` re-resolving the base
  tag — escalate before changing the default, since a per-build registry round trip is
  a product tradeoff rather than an implementation detail.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Stale base image because the default build no longer pulls (C1) | User builds against an outdated image without knowing | `dcc build --update` re-pulls; documented in help text and README. Escalate if a workflow needs the old per-build pull |
| `devcontainer.lock` appears unexpectedly in user repos (C2) | Surprise file, possibly committed | Skip the write when entries are empty and no lockfile exists |
| Slower `dcc build` for the simplest profiles | Perceived regression | Two-line Dockerfile over a local image; Docker caches the layer. Measure once in the smoke test |
| Missing-label warning now fires where it never did | Warning noise on pre-existing images | Correct by design — those images genuinely lack the stamp. One rebuild clears it |
| Removal misses a call site and breaks a command | Runtime failure | `grep -rn uses_fast_path` in the acceptance criteria; full clippy with `--all-targets` |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| The generated Dockerfile for a fast-path config is `FROM` + `LABEL` only | High | Asserted today by `dockerfile_root_user_skips_creation` in `src/features/context.rs` |
| No code path breaks on empty features / root user | High | Traced through `build_base_image`, `build_dcc_stage`, `resolve_features`, `run_build_preparation`; all guarded |
| `docker::pull`/`tag` have no other callers | High | Confirmed by grep; only `build.rs:145` and `build.rs:148` |
| No import is orphaned in `stop.rs` | High | `CacheDir` and `config` remain used by the dry-run branch |

## Verification Plan

- Automated checks: `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo build`.
- Unit tests: `version_warning` with no fast-path parameter (match/mismatch/missing);
  build-flow tests that previously asserted fast-path selection are deleted, not
  weakened.
- Ignored Docker smoke tests: `dcc build` on a plain root+image profile succeeds and the
  resulting image carries `dcc.version`; `dcc exec` works on it end to end;
  `dcc build --no-cache` is honored; whichever freshness behavior C1 selects.
- Manual checks: `dcc --debug build` no longer prints a fast-path line.
- Documentation checks: no stale fast-path prose in README or architecture.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r1 | 2026-08-13 | T-0025 r3 design (D0) | Initial intake | — | — |
| r2 | 2026-08-13 | User acceptance of the recommended approaches | C1, C2, and C3 settled: `--pull` under `--update`; skip the empty lockfile write; delete `docker::pull`/`tag` | The three open consequences needed product agreement before implementation could start | Acceptance criteria now name the specific required behavior for each; no open decisions remain |

## Done When

- `uses_fast_path` and its branch are gone, with no residual call sites.
- Every image dcc builds carries a `dcc.version` stamp.
- C1, C2, and C3 are implemented as decided and recorded in a decision record.
- Documentation matches the implementation and required checks pass.
