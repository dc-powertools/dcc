# T-0027: Remove the `image` fast path (`uses_fast_path`) so every dcc container is uniformly built from a dcc image stage, and drop the fast-path special cases in `build.rs`, `version.rs`, `run.rs`, `stop.rs`, `exec.rs`, and `uid.rs`.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0027

## Outcome

Remove the `image` fast path (`uses_fast_path`) so every dcc container is uniformly built from a dcc image stage, and drop the fast-path special cases in `build.rs`, `version.rs`, `run.rs`, `stop.rs`, `exec.rs`, and `uid.rs`.

## Authority / Revision

User direction 2026-08-13 / r2

## Status

Done

## Depends On

None

## Route / Risk

Initiative / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Deleted `uses_fast_path` and its pull+tag branch in `build.rs`; every build now runs `build_base_image` + `build_dcc_stage`, so every image carries a `dcc.version` label. C1: added `pull: bool` to `DockerBuildOptions`, emit `--pull` in `build_args` when set, plumbed `opts.update`→`pull` through `build_base_image` (build-source path) and `build_dcc_stage` (dcc stage); `--update` help text documents the base-image re-pull. C2: `write_lockfile` skips the write when `lock_entries` is empty and no lockfile exists (an existing lockfile is still rewritten). C3: deleted `docker::pull`/`docker::tag` (fast path was their only caller). Dropped `current_uses_fast_path` param from both `version.rs` entry points; `version_warning` collapses three `None` arms into one (missing label now always warns). Removed fast-path plumbing in `exec.rs`, `run.rs`, `stop.rs` (incl. the local `current_uses_fast_path` helper), and the vacuous `fast_path_config_implies_no_remap` test in `uid.rs`. `--no-cache` now honored for all profiles (fast path used to discard it). Deleted 10 tests (7 fast-path + 2 version + 1 uid), added 2 `--pull` flag tests. Updated docker_smoke `fast_path_root_profile_unaffected_by_uid_remap`→`root_image_profile_builds_with_version_stamp_and_no_remap` (now asserts the version label). Updated README + architecture fast-path prose. 470 unit tests, 29 integration, 33 ignored Docker smokes; fmt, clippy (`--all-targets -D warnings`), check, test, build all clean. `rt` bind mount unchanged (T-0028 tracks the delivery-model question).

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [decision 0003 remove image fast path](decision-0003-remove-image-fast-path.md) — Historical decision rationale.
- [remove image fast path brief](remove-image-fast-path-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0027-remove-image-fast-path/remove-image-fast-path-brief.md`; `.meta/archive/tasks/0027-remove-image-fast-path/decision-0003-remove-image-fast-path.md`.

## Provenance

Migrated from the T-0027 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
