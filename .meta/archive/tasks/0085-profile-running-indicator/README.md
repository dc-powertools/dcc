# T-0085: Implement the approved T-0084 `dcc profile list` running-container indicator through batched Docker status, runtime-role classification, stable text/JSON output, offline degradation, tests, and documentation.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0085

## Outcome

Implement the approved T-0084 `dcc profile list` running-container indicator through batched Docker status, runtime-role classification, stable text/JSON output, offline degradation, tests, and documentation.

## Authority / Revision

User approval plus required internal-label and dry-run integrity corrections 2026-09-08 / r3

## Status

Done

## Depends On

T-0084

## Route / Risk

Initiative / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added one batched label-filtered running-container snapshot, `[running]` text composed after `(default)`, stable tri-state JSON, runtime/build-prep role labels with legacy fallback, truthful unavailable/unknown behavior, escaped diagnostics, reserved internal-label protection, and zero-query empty/dry-run paths. Focused and full format/check/Clippy/test/build verification passed: 568 unit and 87 runnable integration tests, with the new live Docker smoke compiled but not executed because Docker is absent. Independent review found no remaining issues.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [profile running indicator notes](profile-running-indicator-notes.md) — Scope, design or execution evidence.
- [profile running indicator design](../0084-profile-running-indicator-design/profile-running-indicator-design.md) — Scope, design or execution evidence; canonical owner T-0084.

Additional original references: `.meta/archive/tasks/0084-profile-running-indicator-design/profile-running-indicator-design.md`; `.meta/archive/tasks/0085-profile-running-indicator/profile-running-indicator-notes.md`.

## Provenance

Migrated from the T-0085 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
