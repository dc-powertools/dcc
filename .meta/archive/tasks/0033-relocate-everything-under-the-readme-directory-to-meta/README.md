# T-0033: Relocate everything under the `readme` directory to `.meta` and make every internal link and reference consistent with the new path.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0033

## Outcome

Relocate everything under the `readme` directory to `.meta` and make every internal link and reference consistent with the new path.

## Authority / Revision

User request 2026-08-13 / r1

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

`git mv readme .meta` (62 files, rename-detected, history preserved), then rewrote every `readme/` path reference to `.meta/` across the 44 files that carried one: all of `.meta/**`, root `AGENTS.md`, and two Rust doc comments (`src/features/context.rs`, `src/seed.rs`). Preserved five historical mentions of the pre-framework `readme/DEVELOPMENT.md`, `readme/STYLE.md`, `readme/ARCHITECTURE.md`, and the unexpected-`readme/`-files finding in T-0002/T-0003 rows and source-map S-0005/S-0006/S-0007 provenance, because those named files never existed at `.meta`. The framework core keeps its `<root>/meta/` contract, so the reusable tree is now `.meta/meta/`. Verified every relative Markdown link in `.meta`, `AGENTS.md`, and `README.md` resolves (0 broken); the only unresolved backtick paths are pre-existing `NNNN-` schema placeholders and not-yet-created optional artifacts. No references exist in `.gitignore`, `.github/`, `.devcontainer/`, `scripts/`, `install.sh`, `Cargo.toml`, or the product `README.md`. Verified `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`, and `cargo test` (479 unit, 29 CLI, 9 config, 6 feature; 33 Docker smokes still ignored).

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0033 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
