# Task Catalog

This is the canonical discovery and lifecycle record for every accepted task. Physical
row order has no scheduling meaning. The Root Orchestrator is the sole writer.

- Format: 1
- Next task ID: T-0003
- Primary task: None
- Scheduling: Running
- Global pause source or reason: None

## Tasks

| ID | Outcome | Authority / Rev | Status | Depends On | Route / Risk | Approval Or Blocker | Next Safe Action | Details | Result |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T-0001 | Load agent files and initialize the development framework state. | User request 2026-07-14 / r1 | Done | None | Discover / Low | None | Stop; outcome complete. | None | Created the project cursor, task catalog, project brief, implementation context, source map, glossary, and standards. Verified `cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build`, and `cargo run -- --help`. |
| T-0002 | Evaluate `AGENTS.bak.md` and unexpected `readme/` files, merge durable guidance into framework state, and remove leftovers. | User request 2026-07-14 / r1 | Done | None | Quick change / Low | None | Stop; outcome complete. | None | Migrated the backup's project-doc load rules, `.gitignore` boundary, and Codex question-handling guidance into project context, standards, and source map. Removed `AGENTS.bak.md`. Confirmed there are no untracked files. |

## Operating Contract

- Lifecycle, ownership, selection, detail, and archive rules:
  `readme/meta/knowledge-management.md#task-lifecycle-and-selection`
- Intake and task-close behavior: `readme/meta/root-loop.md`
- Interruption and message-targeting behavior: `readme/meta/resumption-protocol.md`
- This file owns only project task facts. Do not add another work or priority list.
