# T-0041: Remove lockfile implementation because Feature-backed installs are not version-locked enough for lockfiles to add much value.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0041

## Outcome

Remove lockfile implementation because Feature-backed installs are not version-locked enough for lockfiles to add much value.

## Authority / Revision

User request 2026-08-24 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Removed `devcontainer.lock` read/write behavior, lock entry generation, locked OCI digest plumbing, and local Feature digest storage; OCI downloads still verify the resolved blob digest fetched from the manifest. Updated `--update` help and docs to describe base-image refresh only. Verified `cargo check`, stale lockfile reference scan, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (483 unit, 29 CLI with 3 ignored, 9 config, 33 Docker smokes ignored, 6 feature), and `cargo build`.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0041 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
