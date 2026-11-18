# T-0042: Fully remove the `dcc build --update` flag and make `--no-cache` also re-pull base images.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0042

## Outcome

Fully remove the `dcc build --update` flag and make `--no-cache` also re-pull base images.

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

Removed the `build --update` CLI flag, dispatch field, and build option plumbing. `--no-cache` now passes Docker `--pull` when building from an upstream base image: directly for image profiles and during the official `build` source's base-image build, while avoiding `--pull` for the generated local intermediate base tag. Updated docs and architecture, and added CLI coverage that `--update` is rejected. Verified `cargo check`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (483 unit, 30 CLI with 3 ignored, 9 config, 33 Docker smokes ignored, 6 feature), `cargo build`, stale `--update` scan, and `git diff --check`.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0042 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
