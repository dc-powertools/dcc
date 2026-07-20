# Project State

Read this file at the start of every session. It is the bounded project-wide cursor and
documentation index, not a task list or history log.

## Task Cursor

- Task catalog: `readme/tasks/README.md`
- Primary task: None
- Primary details: None

The task catalog owns outcomes, status, dependencies, task-specific approvals or
blockers, next actions, detail links, and results. Do not copy them here.

## Global Parked Approvals

| ID | Gated Action | Status | Affected Tasks | Decision Record |
| --- | --- | --- | --- | --- |
| None | | | | |

## Known Global Dead Ends

- None.

## Recently Completed

| Date | Outcome | Durable Record |
| --- | --- | --- |
| 2026-07-20 | Reorganized README lifecycle-hook documentation into a dedicated trigger table. | `readme/tasks/README.md#tasks` |
| 2026-07-20 | Removed `initializeCommand` execution so the hook is only parsed with an unsupported warning. | `readme/tasks/README.md#tasks` |
| 2026-07-20 | Updated user-facing README guidance to match the completed `dcc` rewrite behavior and current CLI design. | `readme/tasks/README.md#tasks` |
| 2026-07-20 | Added expanded ignored Docker behavior tests for lifecycle phases, state persistence, durable/one-shot reuse, workspaceFolder, env substitution, and Feature metadata. | `readme/tasks/README.md#tasks` |
| 2026-07-20 | Added `--dry-run` and `--format json` support for Docker-free CLI validation and converted fragile CLI acceptance checks to dry-run assertions. | `readme/tasks/README.md#tasks` |
| 2026-07-20 | Added a GitHub-hosted Docker smoke test workflow job while keeping Docker tests ignored for local development-container test runs. | `readme/tasks/README.md#tasks` |
| 2026-07-15 | Installed validation tooling and completed official Dev Container CLI config validation for the `dcc` rewrite. | `readme/quality/0004-dcc-rewrite-quality.md` |
| 2026-07-14 | Completed final `dcc` compatibility, validation records, and parent rewrite closure. | `readme/tasks/README.md#tasks` |
| 2026-07-14 | Implemented durable runtime lifecycle commands and one-shot bookkeeping. | `readme/tasks/README.md#tasks` |
| 2026-07-14 | Implemented official build-source support, generated controller/hook assets, and build preparation. | `readme/tasks/README.md#tasks` |
| 2026-07-14 | Implemented Feature metadata compatibility for commands, state, unsupported properties, and unsafe runtime gating. | `readme/tasks/README.md#tasks` |
| 2026-07-14 | Implemented validated `customizations.dcc.state` path handling and profile-local state mount planning. | `readme/tasks/README.md#tasks` |
| 2026-07-14 | Implemented schema-compatible `customizations.dcc` config parsing and merge behavior. | `readme/tasks/README.md#tasks` |
| 2026-07-14 | Migrated development, style, and architecture docs into framework-owned project files. | `readme/tasks/README.md#tasks` |
| 2026-07-14 | Merged backup agent guidance into framework state and removed `AGENTS.bak.md`. | `readme/tasks/README.md#tasks` |
| 2026-07-14 | Initialized the framework project cursor, task catalog, and project memory. | `readme/tasks/README.md#tasks` |

## Documentation Map

- Reusable framework: `readme/meta/README.md`
- Task catalog: `readme/tasks/README.md`
- Rewrite quality record: `readme/quality/0004-dcc-rewrite-quality.md`
- Runtime threat model: `readme/threat-models/0004-dcc-runtime.md`
- Product brief: `readme/project/brief.md`
- Implementation context: `readme/project/context.md`
- Project standards and command catalog: `readme/project/standards.md`
- Detailed architecture: `readme/project/architecture.md`
- Detailed development guide: `readme/project/development.md`
- Detailed Rust style guide: `readme/project/rust-style.md`
- Source map: `readme/project/source-map.md`
- Glossary: `readme/project/glossary.md`

## Hygiene

- Last consistency and pruning pass: 2026-07-14
- Completed repository-changing tasks since that pass: 15
- Next pass due: 2026-08-13 or after 10 completed repository-changing tasks, whichever
  comes first
