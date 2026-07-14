# Task Catalog

This is the canonical discovery and lifecycle record for every accepted task. Physical
row order has no scheduling meaning. The Root Orchestrator is the sole writer.

- Format: 1
- Next task ID: T-0011
- Primary task: None
- Scheduling: Running
- Global pause source or reason: None

## Tasks

| ID | Outcome | Authority / Rev | Status | Depends On | Route / Risk | Approval Or Blocker | Next Safe Action | Details | Result |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-0001 | Load agent files and initialize the development framework state. | User request 2026-07-14 / r1 | Done | None | Discover / Low | None | Stop; outcome complete. | None | Created the project cursor, task catalog, project brief, implementation context, source map, glossary, and standards. Verified `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build`, and `cargo run -- --help`. |
| T-0002 | Evaluate `AGENTS.bak.md` and unexpected `readme/` files, merge durable guidance into framework state, and remove leftovers. | User request 2026-07-14 / r1 | Done | None | Quick change / Low | None | Stop; outcome complete. | None | Migrated the backup's project-doc load rules, `.gitignore` boundary, and Codex question-handling guidance into project context, standards, and source map. Removed `AGENTS.bak.md`. Confirmed there are no untracked files. |
| T-0003 | Migrate `readme/DEVELOPMENT.md`, `readme/STYLE.md`, and `readme/ARCHITECTURE.md` to strict framework ownership. | User request 2026-07-14 / r1 | Done | None | Quick change / Low | None | Stop; outcome complete. | None | Moved the legacy docs to `readme/project/development.md`, `readme/project/rust-style.md`, and `readme/project/architecture.md`; updated cursor, context, standards, source map, glossary, and internal references so no unmanaged legacy doc paths remain. |
| T-0004 | Coordinate the full `dcc` rewrite for devcontainer schema-compatible config, state persistence, durable container lifecycle commands, and shell-oriented attach behavior. | User request 2026-07-14 + attach amendment + full-framework execution request / r3 | Parked | Child tasks T-0005 through T-0010 | Initiative / High | None | Resume for final consistency, release-readiness review, and parent closure after child tasks complete. | `readme/tasks/0004-dcc-rewrite-brief.md`; `readme/tasks/0004-dcc-rewrite-notes.md`; `readme/quality/0004-dcc-rewrite-quality.md`; `readme/threat-models/0004-dcc-runtime.md` | Decomposed into child tasks T-0005 through T-0010 for self-contained implementation and commits. |
| T-0005 | Implement devcontainer schema-compatible config parsing and merge behavior under `customizations.dcc`, including legacy `extends`/`scripts` warnings. | Parent T-0004 / r1 | Done | None | Initiative / High | None | Stop; outcome complete. | `readme/tasks/0005-config-schema-brief.md` | Added `customizations.dcc.extends`, `commands`, and parser-level `state` support; normalized legacy top-level `extends`/`scripts` with deprecation warnings; preserved existing command resolution semantics; verified fmt, clippy, tests, and build; completed read-only specialist review with no blocking findings. |
| T-0006 | Implement validated `customizations.dcc.state` path model and profile-local state cache mount planning. | Parent T-0004 / r1 | Done | T-0005 | Initiative / High | None | Stop; outcome complete. | `readme/tasks/0006-state-cache-brief.md` | Added state path substitution/validation, duplicate/conflict and overlap rejection, deferred runtime `${containerEnv:VAR}` resolution, profile-local state mount planning, host preparation for directory/file state, runtime/debug mount integration, docs, and tests. Verified fmt, clippy, tests, and build. |
| T-0007 | Update Feature metadata handling for state, commands, unsupported properties, unsafe runtime settings, and lifecycle hook collection order. | Parent T-0004 / r1 | Ready | T-0005, T-0006 | Initiative / High | None | Create a T-0007 brief and delegate the implementation-heavy Feature metadata slice with disjoint ownership. | None | None |
| T-0008 | Implement build preparation: official `build` source support, generated controller/hook assets, default prep hooks, and `build --refresh-only`. | Parent T-0004 / r1 | Pending | T-0005, T-0006, T-0007 | Initiative / High | None | Start after Feature/runtime metadata is available. | None | None |
| T-0009 | Implement durable runtime lifecycle commands: `start`, `stop`, `run`, `exec`, `attach`, one-shot bookkeeping, and `--keep` promotion. | Parent T-0004 / r1 | Pending | T-0008 | Initiative / High | None | Start after build/controller assets exist. | None | None |
| T-0010 | Complete port attributes, safe `runArgs`, documentation, fixtures, official config validation, strict review, and final parent closure. | Parent T-0004 / r1 | Pending | T-0005, T-0006, T-0007, T-0008, T-0009 | Initiative / High | None | Start after core behavior is implemented. | None | None |

## Operating Contract

- Lifecycle, ownership, selection, detail, and archive rules:
  `readme/meta/knowledge-management.md#task-lifecycle-and-selection`
- Intake and task-close behavior: `readme/meta/root-loop.md`
- Interruption and message-targeting behavior: `readme/meta/resumption-protocol.md`
- This file owns only project task facts. Do not add another work or priority list.
