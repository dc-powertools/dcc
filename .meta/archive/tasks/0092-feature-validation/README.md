# T-0092: Add offline Feature metadata validation using a pinned upstream schema and explicit dcc extensions, sharing mount parsing with project configuration and preserving Feature editing.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0092

## Outcome

Add offline Feature metadata validation using a pinned upstream schema and explicit dcc extensions, sharing mount parsing with project configuration and preserving Feature editing.

## Authority / Revision

User request and shared-parser amendment 2026-09-28 / r2

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

Added single/collection validation, bundled upstream draft-07 schema and explicit extensions, upstream-only mode, file/JSON diagnostics, parser compatibility checks, and shared project/Feature/image-label mount support preserving readonly strings. Existing edits remain compatible. Format, locked check, all-target Clippy, 592 unit plus 114 runnable integration tests (38 ignored as designed), build, schema provenance, and focused diff/security review passed. Publication remains the official CLI's responsibility.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality feature validation](quality-feature-validation.md) — Verification and review evidence.
- [feature validation](feature-validation.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0092-feature-validation/feature-validation.md`; `.meta/archive/tasks/0092-feature-validation/quality-feature-validation.md`.

## Provenance

Migrated from the T-0092 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
