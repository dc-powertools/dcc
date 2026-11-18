# T-0029: Bake the supervisor scripts (`dcc-supervisor`, `dcc-ctl`, `dcc-exec`) into the image via the build context, retarget the entrypoint and exec paths off the `rt` mount, shrink `RtDir` to manage only `start-hooks/`, and handle the build-prep container's now-possibly-empty mount.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0029

## Outcome

Bake the supervisor scripts (`dcc-supervisor`, `dcc-ctl`, `dcc-exec`) into the image via the build context, retarget the entrypoint and exec paths off the `rt` mount, shrink `RtDir` to manage only `start-hooks/`, and handle the build-prep container's now-possibly-empty mount.

## Authority / Revision

Decision 0004 (T-0028) / r1

## Status

Done

## Depends On

T-0028, T-0030

## Route / Risk

Initiative / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added `baked_supervisor_assets()` to `src/supervisor.rs` returning the three scripts as `.dcc-generated/` context files; `generated_assets()` in `src/features/mod.rs` now includes them so `COPY .dcc-generated/ /usr/local/share/dcc/` bakes them (always-on). Added `DCC_SHARE` const; retargeted `--entrypoint`, `dcc-ctl`, `dcc-exec` paths from `RT_MOUNT/dcc-*` to `DCC_SHARE/dcc-*` in `exec.rs`, `stop.rs`, `build.rs`, and the generated shell scripts. `RtDir::materialize` no longer writes the three scripts — only creates the dir and clears stale `start-hooks/`; the `rt` bind mount survives for `start-hooks/` only. Updated supervisor.rs docs, exec.rs debug output, and 3 supervisor tests. 479 unit tests; fmt, clippy, test, build clean.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [decision 0004 embed supervisor in image](../0028-supervisor-delivery-model/decision-0004-embed-supervisor-in-image.md) — Historical decision rationale; canonical owner T-0028.

Additional original references: `.meta/archive/tasks/0028-supervisor-delivery-model/decision-0004-embed-supervisor-in-image.md`.

## Provenance

Migrated from the T-0029 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
