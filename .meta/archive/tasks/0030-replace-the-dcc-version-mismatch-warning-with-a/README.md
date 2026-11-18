# T-0030: Replace the `dcc.version` mismatch warning with a semver compatibility gate for runtime commands (patch-level drift proceeds; major/minor drift or a missing label refuses with the rebuild command), and record the "no host↔supervisor protocol change in a patch release" rule in project standards.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0030

## Outcome

Replace the `dcc.version` mismatch warning with a semver compatibility gate for runtime commands (patch-level drift proceeds; major/minor drift or a missing label refuses with the rebuild command), and record the "no host↔supervisor protocol change in a patch release" rule in project standards.

## Authority / Revision

Decision 0004 (T-0028) / r1

## Status

Done

## Depends On

T-0028

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Rewrote `src/version.rs`: replaced `warn_if_image_version_mismatch`/`_best_effort`/`version_warning` with `ensure_image_version_compatible`/`_best_effort` and a `SemVer` type (no new dependency — minimal `major.minor.patch` parser that strips `v` prefix and ignores pre-release/build metadata). Compatibility: equal or patch-only drift proceeds; major/minor drift or missing label returns an error naming the rebuild command. `dcc build` is exempt. Updated callers in `exec.rs`, `run.rs`, `stop.rs`. Added 10 semver tests. Recorded the patch-stable-protocol rule in `.meta/project/standards.md`. 479 unit tests; fmt, clippy, test, build clean.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [decision 0004 embed supervisor in image](../0028-supervisor-delivery-model/decision-0004-embed-supervisor-in-image.md) — Historical decision rationale; canonical owner T-0028.

Additional original references: `.meta/archive/tasks/0028-supervisor-delivery-model/decision-0004-embed-supervisor-in-image.md`.

## Provenance

Migrated from the T-0030 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
