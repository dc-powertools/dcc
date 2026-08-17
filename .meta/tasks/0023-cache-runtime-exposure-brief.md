# T-0023 Brief: Container-Writable `dcc` Runtime Bookkeeping Under `/cache`

## Identity And Source

- Task ID: T-0023
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Accepted in-scope finding during durable cache mounting review
- Source reference and date: Durable cache mounting review, 2026-07-21
- Parent or split task IDs: None

## Goal

Container-side code cannot corrupt or manipulate `dcc`'s own host-side runtime
bookkeeping, and the trust boundary around the profile cache mount is documented and
covered by the runtime threat model.

## Background

`dcc` mounts the whole profile cache directory into the container:

```
-v <workspace>/.dcc/<profile>:/cache
```

(`src/exec.rs` runtime launch and `build_prep_container_args` in `src/build.rs`.)

The profile cache directory is also where `dcc` keeps host-side state that it trusts:

- `RuntimeState` roots at `cache_dir.host_path.join("runtime")` (`src/runtime.rs`), holding
  the container mode file, the `active` command records, and the `lock` directory.
- T-0022 introduces seeded state under `.dcc/<profile>/state/...`.

Because the mount is the cache **root**, all of this appears inside the container at
`/cache/runtime` and `/cache/state`, read-write, for whatever user the container runs as.
Container-side code can therefore rewrite `dcc`'s durable/one-shot mode, delete or forge
active-command records, or hold the runtime lock.

This matters because `dcc` explicitly treats the container as a lower-trust zone
elsewhere: `--tmpfs /workspace/.dcc` is applied precisely so the container cannot see or
modify the host `.dcc/` directory through the workspace mount. The `/cache` mount
reintroduces the same exposure through a different route, which appears to be unintended
rather than a considered tradeoff.

`.meta/threat-models/0004-dcc-runtime.md` covers state paths pointing at system
directories and generated-script quoting, but it does not cover container-side write
access to `dcc`'s own runtime bookkeeping.

Consequences worth noting, in ascending severity:

1. Corrupted bookkeeping breaks teardown logic — a forged active-command record keeps a
   one-shot container alive, or a deleted one causes premature teardown while a command is
   still running.
2. Flipping the mode file to `durable` defeats automatic cleanup.
3. Lock manipulation can stall host-side `dcc` invocations up to the timeout.

This is a local-developer-tool trust boundary, not a remote attack surface: the realistic
trigger is a compromised dependency, a hostile Feature, or a coding agent running in the
container, rather than a network attacker.

## Scope

In scope:

- Decide and implement the containment approach (see Design To Complete).
- Preserve the documented `/cache` contract for user data and `${containerCacheFolder}`.
- Update `.meta/threat-models/0004-dcc-runtime.md` with this threat, its control, and
  residual risk.
- Update README and `.meta/project/architecture.md` where they describe `/cache` and the
  runtime bookkeeping location.
- Tests covering the chosen containment.

Out of scope:

- Critical-path guards for declared state; T-0021 owns those (it separately blocks
  `/cache` as a *state* target, which does not address this mount-level exposure).
- Seeding behavior; T-0022 owns that.
- Any broader container-escape hardening or sandboxing model.

## Design To Complete

The implementer must choose among these, all of which keep the user-facing `/cache`
contract intact. The options are ordered by increasing structural change:

| Option | Approach | Pros | Cons |
| --- | --- | --- | --- |
| A. Mask subpaths | Keep `-v .../<profile>:/cache`, add `--tmpfs /cache/runtime` (mirroring the existing `--tmpfs /workspace/.dcc` idiom) | Smallest change; reuses an established pattern; no layout migration | Masks rather than relocates; a tmpfs at `/cache/runtime` is itself container-writable, just not host-backed; needs one tmpfs per protected subpath |
| B. Relocate bookkeeping | Move runtime bookkeeping out of the mounted directory, e.g. `.dcc/<profile>.runtime/` as a sibling, mirroring the T-0022 ledger placement at `.dcc/<profile>.seed.json` | Removes the exposure at its root; consistent with the ledger decision; no masking needed | Changes an on-disk path; needs a migration or cleanup path for existing `.dcc/<profile>/runtime` directories |
| C. Narrow the mount | Mount only a dedicated user-data subdirectory as `/cache` | Cleanest boundary | Changes what `/cache` means on the host; risks breaking existing caches users already populated via `${containerCacheFolder}` |

Recommendation: **B**, because it removes the exposure rather than papering over it and
because T-0022 already establishes the `.dcc/<profile>.<name>` sibling convention for
host-trusted artifacts, making the two consistent. B also avoids per-subpath masking as
more host-side artifacts are added. If migration cost proves unacceptable, A is an
acceptable interim with the residual risk recorded.

Whichever option is chosen, note that `.dcc/<profile>/state/...` remains inside `/cache`
by design: seeded state is *meant* to be container-visible and container-writable, so it is
not part of this exposure. Only `dcc`'s own control-plane bookkeeping needs containment.

Remaining implementer decisions: option choice; whether to migrate or simply ignore and
clean up stale `.dcc/<profile>/runtime` directories; and whether `dcc stop` should tolerate
bookkeeping at either location during a transition.

## Users And Workflows

| User/Actor | Workflow | Expected Change |
| --- | --- | --- |
| Developer | `dcc run`, `dcc start`, `dcc stop` | No visible change; teardown and reuse behave as today |
| Developer using `${containerCacheFolder}` | Writes to `/cache/...` from inside the container | Unchanged; user cache data still works |
| Container-side code, hostile dependency, or in-container agent | Attempts to write `dcc` runtime bookkeeping | No longer able to reach host-backed bookkeeping |
| Security reviewer | Reads the threat model | Finds this trust boundary documented with its control |

## Acceptance Criteria

- [ ] `dcc`'s runtime mode, active-command records, and lock are not writable from inside
      the container through the `/cache` mount.
- [ ] `${containerCacheFolder}` / `/cache` remains usable for user cache data.
- [ ] Durable vs one-shot reuse, active-command bookkeeping, and `dcc stop` teardown
      behave as before the change.
- [ ] If bookkeeping relocates, a pre-existing `.dcc/<profile>/runtime` directory does not
      break the new code path.
- [ ] Seeded state under `.dcc/<profile>/state` remains container-visible and writable.
- [ ] `.meta/threat-models/0004-dcc-runtime.md` records the threat, control, and residual
      risk.
- [ ] README and architecture docs match the implemented layout.
- [ ] Ignored Docker smoke coverage asserts the bookkeeping is unreachable from inside the
      container while `/cache` writes still work.

## Constraints

- Do not regress the existing `--tmpfs /workspace/.dcc` masking.
- Keep the change independent of T-0021 and T-0022 so it can land on its own; if T-0022
  lands first, stay consistent with its `.dcc/<profile>.seed.json` placement.
- Apply the change to **both** the runtime launch path (`src/exec.rs`) and the
  build-preparation container (`src/build.rs`); they construct mounts separately and both
  currently mount the cache root.
- Docker-dependent tests are `#[ignore]` and run in CI only
  (`.meta/project/standards.md`).
- `anyhow::Result` with `.with_context`; no `unwrap`/`expect` outside `#[cfg(test)]`.

## Workflow Route Rationale

- Cataloged route and risk: See this task's catalog row.
- Why this route: Small surface but a security-relevant trust boundary, touching runtime
  bookkeeping that governs container teardown.
- Why this risk gate: An error here breaks container reuse or teardown, and the change is
  motivated by a security finding, so it needs threat-model and review evidence rather
  than tests alone.
- Upstream artifacts required: `.meta/threat-models/0004-dcc-runtime.md`.
- Escalation trigger: If the chosen option changes the meaning of `/cache` for existing
  users, escalate before implementing — that is a user-visible compatibility decision, not
  an implementation detail.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Relocating bookkeeping orphans state for an already-running container | Stale containers that `dcc stop` cannot clean up | Tolerate both locations during a transition, or detect and clean up the legacy path |
| Masking with tmpfs hides a real host directory that later code expects | Confusing failures | Prefer relocation (option B); if masking, assert the host path is unused by host-side code |
| Narrowing the mount breaks user caches populated via `${containerCacheFolder}` | User data appears to vanish | Do not choose option C without an explicit compatibility decision |
| Change diverges between the runtime and build-prep paths | Exposure persists in one path | Single shared helper for cache mount construction; test both call sites |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| Nothing inside the container legitimately needs to read `dcc` runtime bookkeeping | High | Grep generated controller, command-wrapper, and hook assets for `runtime` references |
| Only host-side `dcc` writes the bookkeeping today | High | `src/runtime.rs` is the sole writer |
| `/cache/state` container-writability is intended | High | Seeded state is meant to be used and updated by container tooling |

## Verification Plan

- Automated checks: `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`,
  `cargo test`, `cargo build`.
- Unit tests: cache and bookkeeping path resolution, mount argument construction for both
  the runtime and build-prep paths, and legacy-path tolerance if relocation is chosen.
- Ignored Docker smoke tests: bookkeeping unreachable from inside the container; `/cache`
  user writes still work; durable vs one-shot reuse and `dcc stop` teardown unchanged.
- Manual checks: `dcc --debug run` mount output shows the intended layout.
- Documentation checks: threat model, README, and architecture updated consistently.
- Baseline evidence: write a Docker smoke test that writes to `dcc`'s bookkeeping from
  inside the container and observe it **succeed** against current code, demonstrating the
  exposure before the fix.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r1 | 2026-07-21 | Accepted in-scope finding | Initial intake | — | — |

## Done When

- Container-side code cannot reach `dcc`'s host-backed runtime bookkeeping.
- Existing cache, reuse, and teardown behavior is preserved with observed evidence.
- The threat model records the boundary, control, and residual risk.
- Required checks pass and documentation matches the implementation.
