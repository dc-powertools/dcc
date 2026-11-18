# T-0039: Reorganize public documentation: keep `README.md` as an end-user overview, move feature detail to `docs/features.md`, move development guidance to `docs/development.md`, and link those pages from the README.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0039

## Outcome

Reorganize public documentation: keep `README.md` as an end-user overview, move feature detail to `docs/features.md`, move development guidance to `docs/development.md`, and link those pages from the README.

## Authority / Revision

User request 2026-08-17 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / Low

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added `docs/features.md` and `docs/development.md`, shortened `README.md` to an end-user overview with links to both docs, moved detailed feature/configuration/runtime guidance into the feature guide, moved maintainer/release guidance into the development guide, and corrected stale baked-supervisor wording in public docs plus `.meta/project/architecture.md`. Verified Markdown local links/anchors, `git diff --check`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and `cargo build`.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0039 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
