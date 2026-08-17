# Source Map

| ID | Source | Owner/Publisher | Date Checked | Trust Tier | Scope | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| S-0001 | `AGENTS.md` | Repository | 2026-07-14 | Authority | Agent startup and framework bootstrap instructions | Directs sessions to `.meta/meta/README.md` and onboarding when the cursor or catalog is missing. |
| S-0002 | `.meta/meta/README.md` | Reusable framework | 2026-07-14 | Authority | Framework startup order, directory contract, and bootstrap rules | Reusable process owner, not project fact storage. |
| S-0003 | `.meta/meta/onboarding.md` | Reusable framework | 2026-07-14 | Authority | Onboarding procedure | Used to initialize this project's cursor, catalog, commands, and memory. |
| S-0004 | `README.md`; `docs/features.md`; `docs/development.md` | Repository | 2026-08-17 | Primary Evidence | Public documentation: product overview, supported platforms, install command, user workflows, feature/configuration reference, and human maintainer guidance | Reorganized in T-0039 so README owns the end-user overview, `docs/features.md` owns detailed user-facing behavior, and `docs/development.md` owns public maintainer/release guidance. |
| S-0005 | `.meta/project/architecture.md` | Framework project state | 2026-07-14 | Primary Evidence | Crate structure, modules, dependencies, config semantics, and Docker behavior | Migrated from `readme/ARCHITECTURE.md` for strict framework ownership. |
| S-0006 | `.meta/project/development.md` | Framework project state | 2026-07-14 | Authority | Local development loop, checks, commit expectations, and scope discipline | Migrated from `readme/DEVELOPMENT.md` for strict framework ownership. |
| S-0007 | `.meta/project/rust-style.md` | Framework project state | 2026-07-14 | Authority | Rust coding standards | Migrated from `readme/STYLE.md` for strict framework ownership. |
| S-0008 | `Cargo.toml` | Repository | 2026-07-14 | Primary Evidence | Package metadata, dependencies, dev dependencies, release overflow checks | Manifest source for stack and commands. |
| S-0009 | `.github/workflows/ci.yml` | Repository | 2026-07-14 | Primary Evidence | CI verification commands | Source for fmt, clippy, test, and build commands. |
| S-0010 | `src/` and `tests/` | Repository | 2026-07-14 | Primary Evidence | Current implementation and test behavior | Inspect relevant files before changing code. |
| S-0011 | `AGENTS.bak.md` | Repository backup file | 2026-07-14 | Secondary Evidence | Pre-framework agent guidance | Evaluated and migrated durable guidance into `.meta/project/context.md` and `.meta/project/standards.md`; backup file removed. |
