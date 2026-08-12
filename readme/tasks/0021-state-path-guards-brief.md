# T-0021 Brief: Critical Container Path Guards For Declared State

## Identity And Source

- Task ID: T-0021
- Initial revision: r1
- Catalog: `readme/tasks/README.md`
- Accepted source: User instruction
- Source reference and date: Durable cache mounting review, 2026-07-21
- Parent or split task IDs: None

## Goal

`customizations.dcc.state` rejects container paths whose masking would break the
container or `dcc` itself, with errors that name a supported alternative. Legitimate cache
targets nested beneath broad system directories continue to work.

## Background

State paths are attached as `type=bind` mounts and their host sources are materialized
*empty* before the container starts (`CacheDir::prepare_state_mounts`). Masking a system
path therefore replaces it with an empty directory, and file-kind state replaces a file
with an **empty** file rather than removing it. `{"path": "/etc/passwd", "type": "file"}`
empties the user database and the container fails in a way that does not resemble a
configuration error.

`normalize_state_path` in `src/config/resolve.rs` currently rejects only `/`, `/tmp`,
`/run`, `/proc`, `/sys`, `/dev`, and `/workspace/.dcc`. Two gaps matter:

- **`/cache` is unguarded, which is a live bug.** `/cache` is the profile cache bind mount
  and state host paths live at `.dcc/<profile>/state/...`, so declaring `/cache/state`
  mounts a subtree of the cache directory back into itself. This is recursive and
  confusing today, independent of any seeding work.
- **`/usr/local/share/dcc` is unguarded**, and `dcc` writes its own generated controller,
  command-wrapper, and build-prep hook assets there (`features/context.rs` emits
  `COPY .dcc-generated/ /usr/local/share/dcc/`). Masking it breaks `dcc`'s own assets.

T-0022 makes declared state materially more attractive by seeding it from the image, which
raises the likelihood of users pointing state at broad OS directories. Landing the guards
first keeps that from becoming a support burden.

## Scope

In scope:

- Extend the reserved-path checks in `src/config/resolve.rs` with the two tiers below.
- Apply the guards at **both** validation points: config-load normalization and
  post-`${containerEnv:...}` resolution.
- Error messages that name the rejected path, the reserved path matched, and a supported
  alternative.
- Unit tests for each guarded path, for allowed nested paths, and for the resolved-literal
  path.
- README and `readme/project/architecture.md` updates to the documented reserved list.

Out of scope:

- Seeding or hydration behavior; T-0022 owns that.
- The `/cache/runtime` container-writability exposure; T-0023 owns that.
- Symlink resolution (see Risks); the mitigation belongs to T-0022's hydration container.

## Design To Complete

The existing loop uses `is_path_or_child(&normalized, reserved)`, so every current entry
blocks its whole subtree. That is correct for kernel and binary paths but **wrong** for
broad parents: `/usr/local/cargo`, `/var/cache/apt`, `/home/dev/.cache`, and
`/workspace/target` are all legitimate state targets whose parents are not. The
implementer must therefore introduce a second, exact-match-only tier rather than extending
the existing list.

**Tier 1 — block the path and its entire subtree:**

| Path | Rationale |
| --- | --- |
| `/proc`, `/sys`, `/dev` | Kernel virtual filesystems (already guarded) |
| `/tmp`, `/run` | tmpfs / runtime state (already guarded) |
| `/var/run`, `/var/lock` | Conventionally symlinks into `/run` |
| `/boot` | Kernel images |
| `/bin`, `/sbin`, `/lib`, `/lib32`, `/lib64`, `/libx32` | Core binaries and libraries |
| `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/lib32`, `/usr/lib64`, `/usr/libx32` | merged-`usr` targets of the above |
| `/etc` | Empty-file state corrupts `passwd`, `group`, `shadow`, `nsswitch.conf` |
| `/workspace/.dcc` | tmpfs-masked (already guarded) |
| `/cache` | The profile cache mount itself; self-nesting |
| `/usr/local/share/dcc` | `dcc`'s own generated assets |

`/bin` is a symlink to `/usr/bin` on merged-`usr` distributions, so a textual guard must
list **both** spellings; blocking one does not block the other.

`/etc` is intentionally a whole subtree. Legitimate wants such as persisting `/etc/ssh`
host keys are better served by a lifecycle hook, which runs with mounts attached and is
already the supported mechanism for managing system files.

**Tier 2 — block the exact path only; children stay valid:**

| Path | Blocked because | Must still be allowed |
| --- | --- | --- |
| `/usr` | Masks the entire system tree | `/usr/local/cargo`, `/usr/local/rustup` |
| `/var` | Masks all system state | `/var/cache/apt` |
| `/home` | Masks every user's home | `/home/dev/.cargo` |
| `/root` | Masks root's entire home | `/root/.cargo` |
| `/opt` | Masks all opt trees | `/opt/toolchain/cache` |
| `/workspace` | Shadows the repository bind mount | `/workspace/target` |
| `/srv`, `/mnt`, `/media` | Low value bare; consistency | Subpaths |

`/` is already covered by the existing `segments.is_empty()` check and needs no new entry.

Remaining implementer decisions: the concrete representation of the two tiers (two consts
versus one const of `(path, matching_mode)` pairs) and the exact error wording, subject to
the shape below.

## Users And Workflows

| User/Actor | Workflow | Expected Change |
| --- | --- | --- |
| Developer authoring a profile | Declares `customizations.dcc.state` | A container-breaking path fails at config load with a message naming a supported alternative, instead of producing a subtly broken container |
| Developer caching a toolchain | Declares `/usr/local/cargo` or `/workspace/target` | Unchanged; still accepted |
| Feature author | Ships `customizations.dcc.state` metadata | Same validation applies to Feature-contributed state |
| Coding agent | Reads the error | Message is self-diagnosing without source access |

## Acceptance Criteria

- [ ] Every Tier 1 path rejects both the exact path and a child path.
- [ ] Every Tier 2 path rejects the exact path and accepts a child path.
- [ ] `/cache` and any `/cache/...` child are rejected.
- [ ] `/usr/local/share/dcc` and children are rejected.
- [ ] `/usr/local/cargo`, `/var/cache/apt`, `/home/dev/.cargo`, `/root/.cargo`, and
      `/workspace/target` remain accepted.
- [ ] Both `/bin/...` and `/usr/bin/...` are rejected.
- [ ] A state path that only becomes critical after `${containerEnv:VAR}` resolution
      (e.g. `HOME=/etc`) is rejected at the resolution point.
- [ ] Errors name the rejected path, the matched reserved path, and an alternative.
- [ ] Feature-contributed state is guarded identically to project state.
- [ ] README and architecture reserved-path documentation match the implementation.

## Constraints

- Hard reject; **not** gated behind `--allow-unsafe-runtime`. The existing gated items
  (`privileged`, `capAdd`, `securityOpt`, sensitive mounts) have legitimate uses such as
  docker-in-docker. Masking `/etc` has none — it only breaks the container — so gating it
  would imply a supported mode that does not exist.
- Preserve the existing duplicate, conflicting-kind, and parent/child overlap behavior.
- `anyhow::Result` with `.with_context` at boundaries; no `unwrap`/`expect` outside
  `#[cfg(test)]` (`readme/project/rust-style.md`).
- Error message shape:
  - Subtree: ``customizations.dcc.state path `/etc/passwd` targets reserved system path `/etc`; use a lifecycle hook to manage system files``
  - Exact: ``customizations.dcc.state path `/home` would mask all user homes; declare a specific subdirectory such as `/home/dev/.cache` ``

## Workflow Route Rationale

- Cataloged route and risk: See this task's catalog row.
- Why this route: Localized change to one validation function plus tests and docs; no new
  module, no Docker interaction.
- Why this risk gate: Tightening validation can reject a previously accepted config, so
  the allowed-path assertions matter as much as the rejections.
- Upstream artifacts required: None; this task is independent and should land first.
- Escalation trigger: If any Tier 2 path turns out to need subtree blocking (or the
  reverse) for a real config, re-enter design before broadening the list.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| A previously working config now fails | User-visible regression | Explicit accept-tests for the known-legitimate nested paths; call out the new rejections in README |
| Symlinks defeat textual guards: if the image has `/home/dev/.cache` -> `/etc`, Docker resolves `dst` in the container mount namespace at start, so the guard passes and `/etc` is masked anyway | Broken container despite guards | Not fixable in `normalize_state_path`; document the limit here and let T-0022's hydration container refuse a state path that resolves outside itself |
| Guard applied only at config load | `${containerEnv:HOME}` pointing at a critical path slips through | Apply at both validation points; dedicated test for the resolved-literal case |
| Over-broad `/etc` block frustrates a real use case | Support friction | Error names the lifecycle-hook alternative |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| No existing project fixture or test declares a now-rejected path | Medium | `cargo test` plus a grep for state declarations across `tests/` and `.devcontainer/` |
| Tier 2 paths are never legitimate bare state targets | High | Each has a natural nested alternative documented above |
| `is_path_or_child` semantics suffice for Tier 1 matching | High | Existing reserved-path tests already rely on it |

## Verification Plan

- Automated checks: `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`,
  `cargo test`, `cargo build` (`readme/project/standards.md`).
- New unit tests in `src/config/resolve.rs` covering every Tier 1 path (exact and child),
  every Tier 2 path (exact rejected, child accepted), and the post-resolution literal.
- Manual checks: `cargo run -- --dry-run --format json build` against a profile declaring
  a rejected path shows the validation error.
- Documentation checks: README reserved-path list and
  `readme/project/architecture.md` Cache Management section match the code.
- Baseline evidence: write each new test first and observe it fail against current
  `normalize_state_path` before implementing, per the project's test-first convention.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r1 | 2026-07-21 | User instruction | Initial intake | — | — |

## Done When

- Both guard tiers are implemented at both validation points with the specified error
  shapes.
- All acceptance criteria are satisfied with observed test results.
- Required checks pass and documentation matches the implementation.
