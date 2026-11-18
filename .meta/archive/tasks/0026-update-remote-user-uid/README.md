# T-0026: Implement the spec's `updateRemoteUserUID` (default `true`, Linux-only) so a non-root `containerUser` can write to bind-mounted workspace, cache, and state regardless of host uid; restore `seeded_directory_is_writable_by_container_user` to its original workspace-writing form.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0026

## Outcome

Implement the spec's `updateRemoteUserUID` (default `true`, Linux-only) so a non-root `containerUser` can write to bind-mounted workspace, cache, and state regardless of host uid; restore `seeded_directory_is_writable_by_container_user` to its original workspace-writing form.

## Authority / Revision

User direction 2026-08-13 / r1

## Status

Done

## Depends On

None

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added `updateRemoteUserUID` (recognized config field, default `true`, child-overrides-parent merge) and `src/uid.rs`: `plan_uid_remap` with every reference no-op (root, numeric, already-matching, uid collision, gid-collision-keeps-gid, non-Linux, disabled, host-ids-unavailable), `host_ids` via `id -u`/`id -g`, and `remap_dockerfile_block`/`remap_build_args` baking the `devcontainers/cli` `updateUID.Dockerfile` logic into dcc's generated build stage as a `RUN` after user creation with `--build-arg REMOTE_USER/NEW_UID/NEW_GID`. Wired into `build_dcc_stage` (`docker::build` now forwards build args), `--debug`, and `--dry-run` reports. Restored `seeded_directory_is_writable_by_container_user` to `/workspace/writable.txt` + `fx.read_file`; added ignored Docker smoke tests for non-root writes to `/workspace`/`/cache`/state, in-container uid matching host, and root fast-path unaffected. Updated README and architecture. Seeding interaction resolved: the remap is baked at image build, so T-0022 hydration already sees the remapped `/etc/passwd` and seeded content is owned by the remapped user. Verified fmt, clippy (0 warnings), 463 unit tests, build, dry-run JSON. Live Docker execution delegated to CI.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [decision 0002 update remote user uid in build stage](decision-0002-update-remote-user-uid-in-build-stage.md) — Historical decision rationale.
- [quality update remote user uid quality](quality-update-remote-user-uid-quality.md) — Verification and review evidence.
- [r1 uid remap design](r1-uid-remap-design.md) — Scope, design or execution evidence.
- [update remote user uid brief](update-remote-user-uid-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0026-update-remote-user-uid/update-remote-user-uid-brief.md`; `.meta/archive/tasks/0026-update-remote-user-uid/r1-uid-remap-design.md`.

## Provenance

Migrated from the T-0026 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
