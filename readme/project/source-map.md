# Source Map

| ID | Source | Owner/Publisher | Date Checked | Trust Tier | Scope | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| S-0001 | `AGENTS.md` | Repository | 2026-07-14 | Authority | Agent startup and framework bootstrap instructions | Directs sessions to `readme/meta/README.md` and onboarding when the cursor or catalog is missing. |
| S-0002 | `readme/meta/README.md` | Reusable framework | 2026-07-14 | Authority | Framework startup order, directory contract, and bootstrap rules | Reusable process owner, not project fact storage. |
| S-0003 | `readme/meta/onboarding.md` | Reusable framework | 2026-07-14 | Authority | Onboarding procedure | Used to initialize this project's cursor, catalog, commands, and memory. |
| S-0004 | `README.md` | Repository | 2026-07-14 | Primary Evidence | Product purpose, supported platforms, install command, and user workflows | Product-facing behavior source. |
| S-0005 | `readme/ARCHITECTURE.md` | Repository | 2026-07-14 | Primary Evidence | Crate structure, modules, dependencies, config semantics, and Docker behavior | Technical design source. |
| S-0006 | `readme/DEVELOPMENT.md` | Repository | 2026-07-14 | Authority | Local development loop, checks, commit expectations, and scope discipline | Project-specific development rules. |
| S-0007 | `readme/STYLE.md` | Repository | 2026-07-14 | Authority | Rust coding standards | Project-specific Rust style rules. |
| S-0008 | `Cargo.toml` | Repository | 2026-07-14 | Primary Evidence | Package metadata, dependencies, dev dependencies, release overflow checks | Manifest source for stack and commands. |
| S-0009 | `.github/workflows/ci.yml` | Repository | 2026-07-14 | Primary Evidence | CI verification commands | Source for fmt, clippy, test, and build commands. |
| S-0010 | `src/` and `tests/` | Repository | 2026-07-14 | Primary Evidence | Current implementation and test behavior | Inspect relevant files before changing code. |
| S-0011 | `AGENTS.bak.md` | Repository backup file | 2026-07-14 | Secondary Evidence | Pre-framework agent guidance | Evaluated and migrated durable guidance into `readme/project/context.md` and `readme/project/standards.md`; backup file removed. |
