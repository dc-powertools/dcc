# T-0026 Quality Record

- Date: 2026-08-13
- Change: Implement `updateRemoteUserUID` so a non-root `containerUser` can write
  bind-mounted workspace/cache/state regardless of host uid; restore
  `seeded_directory_is_writable_by_container_user` to its workspace-writing form.
- Route: Initiative
- Risk: High
- Owner or reviewer: Root Orchestrator

## Scope And Criteria

- User-visible outcome: on Linux, a non-root `containerUser` can read and write
  bind-mounted host content (workspace, cache, declared state) without depending on
  the host user's UID happening to match. The remap safely no-ops in every
  reference-defined condition.
- In scope: `updateRemoteUserUID` config property (default `true`), remap planning
  and Dockerfile generation, build debug/dry-run reporting, restored smoke test,
  new non-root smoke tests, README and architecture updates.
- Non-goals: `remoteUser` as a distinct user, Podman `--userns=keep-id`, derived
  `-uid` image tagging (rejected — see `.meta/decisions/0002-…`).

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| `updateRemoteUserUID` is a recognized config property defaulting to `true`. | Unit tests in `config/resolve.rs`. | `test_update_remote_user_uid_defaults_to_true`, `_false_respected`, `_not_in_extra` (strict mode accepts it), `_child_overrides_parent` pass. | Pass |
| On Linux with a non-root `containerUser`, the uid/gid is remapped to the host's before container creation. | `plan_uid_remap` unit tests + Dockerfile generation tests + dry-run report. | `plan_remaps_non_root_named_user` (unix) passes; `dockerfile_emits_remap_block_when_planned` asserts the `ARG`/`RUN` shape and ordering after user creation; local `dcc build --dry-run --format json` reports `updateRemoteUserUID remap planned: user \`dev\` -> uid 1000 gid 1000`. Live Docker execution delegated to CI. | Pass |
| The remap no-ops when: root, numeric, already-matching, uid collision, non-Linux, disabled. | `plan_uid_remap` unit tests. | `plan_skips_when_disabled`, `_skips_root_user`, `_skips_numeric_user`, `_skips_when_host_ids_unavailable`, `fast_path_config_implies_no_remap`, and the non-unix skip branch all pass. The `remap_run_script` carries the inline collision/gid-collision no-op echoes (`remap_block_includes_collision_noop_echo`). | Pass |
| When a group already occupies the target GID, the GID is left unchanged and the UID is still updated. | `remap_run_script` content assertion. | `remap_block_includes_collision_noop_echo` and the script body assert the `EXISTING_GROUP` → `NEW_GID="$OLD_GID"` branch is present. | Pass |
| `seeded_directory_is_writable_by_container_user` restored to `/workspace/writable.txt` + `fx.read_file("writable.txt")`. | Ignored Docker smoke test. | Test rewritten; `#[ignore]`, runs in CI on GitHub-hosted Ubuntu runners. | Pass (pending CI) |
| A non-root `containerUser` can write to `/workspace`, `/cache`, and a declared state path. | Ignored Docker smoke test `non_root_user_writes_workspace_cache_and_state`. | `#[ignore]`, runs in CI. | Pass (pending CI) |
| Seeded state remains writable by the container user after the remap. | Design argument + existing seeding smoke tests. | The remap is baked at image build time, so T-0022 hydration (runs as root, `tar`-preserves image uids) already sees the remapped `/etc/passwd`; seeded content is owned by the remapped user by construction. Recorded in `.meta/decisions/0002-…`. Existing `wiped_dcc_rehydrates_from_image_without_rebuild` and reseed smoke tests continue to pass (ignored, CI). | Pass (pending CI) |
| Build-preparation container and runtime container behave consistently. | Both launch sites use the same `image_tag`, which now carries the remap. | `build_prep_container_args` and `exec.rs` launch from the single remapped `image_tag`; no second tag introduced. | Pass |
| Fast-path profiles (`containerUser: root`) are unaffected, asserted by a test. | `fast_path_config_implies_no_remap` unit test + `fast_path_root_profile_unaffected_by_uid_remap` ignored Docker smoke test. | Unit test passes; smoke test `#[ignore]` in CI. | Pass |
| Existing durable/one-shot reuse, `--keep`, and all `dcc stop` variants still work. | Full `cargo test` (existing supervisor/stop/reuse tests unchanged). | 463 unit + 29 CLI integration tests pass; 24 ignored Docker smoke tests listed. | Pass |
| README and architecture document `updateRemoteUserUID` and correct the `remoteUser`/`containerUser` rows. | Doc review. | README config table has an `updateRemoteUserUID` row and build section describes the remap; architecture module map, struct listings, merge table, and in-memory build context Dockerfile sketch updated. | Pass |
| Required checks pass: `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build`. | Executed in this environment. | All five pass; 463 unit tests, 0 clippy warnings. | Pass |

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | Brief and design doc (`.meta/tasks/0026-r1-uid-remap-design.md`) record accepted requirements, the rejected derived-image alternative, and the seeding-interaction resolution. |
| Verification | Yes (local); CI pending for Docker smoke | All local checks pass. The restored and new Docker smoke tests are `#[ignore]` and run in the GitHub Actions `docker-test` job. |
| Documentation | Yes | README and `.meta/project/architecture.md` updated; decision record `0002` and this quality record added. |
| Residual risk | Live Docker smoke not run locally | Same delegation model as T-0012–T-0024: ignored Docker tests run in CI on GitHub-hosted Ubuntu runners (uid 1001/gid 999), which is exactly the mismatch this task fixes. |

## Required Checks (observed 2026-08-13)

- `cargo fmt --check` — passed, no diff.
- `cargo check` — passed for `dcc v0.1.0`.
- `cargo clippy -- -D warnings` — passed, 0 warnings.
- `cargo test` — passed; 463 unit, 29 CLI flag integration (3 ignored), 9 config error,
  6 feature command, 24 ignored Docker smoke.
- `cargo build` — passed for the dev profile.
