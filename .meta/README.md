# Project State

Read this bounded cursor at session start, then the current-task index and selected
README. Current project/framework documentation owns applicable requirements;
archives supply targeted historical evidence.

## Task Cursor and Global Control

- Current tasks: `.meta/tasks/README.md`
- Primary task: T-0108
- Primary details: `.meta/tasks/0108-persistence-image-ownership/README.md`
- Scheduling: Running
- Global pause source or reason: None

Task READMEs own lifecycle, revision, authority, dependencies, task approvals/blockers,
next action and results. Neither discovery index owns those fields.

## Global Parked Approvals

| ID | Gated Action | Status | Affected Tasks | Evidence |
| --- | --- | --- | --- | --- |
| None | | | | |

## Known Global Dead Ends

- Startup hooks require invoking-process environment substitution at run time;
  do not bake `postStartCommand` into the image. See current architecture guidance.
- Do not bake a duplicate seed store into an image or assume moving payload from an
  earlier layer reclaims its bytes. Same-installation-layer relocation with preserved
  original-path symlinks is allowed by the current persistence contract.

## Recently Completed

| Date | Outcome | Durable Record |
| --- | --- | --- |
| 2026-11-18 | Task-directory migration, separate discovery/archive and random-ID policy completed (T-0120). | `.meta/archive/tasks/0120-task-directory-migration/README.md` |
| 2026-11-17 | Readiness regression UID portability and diagnostics verified with actual host UIDs 1000/0/1001 and 35 Docker smokes (T-0117). | `.meta/archive/tasks/0117-readiness-fixture-uid-portability/README.md` |
| 2026-11-17 | Readiness controls run as supervisor user; regular checks and 34 Docker smokes passed (T-0112). | `.meta/archive/tasks/0112-readiness-fifo-ownership/README.md` |

## Documentation Map

- Reusable framework: `.meta/meta/README.md`
- Current task discovery: `.meta/tasks/README.md`
- Historical task discovery: `.meta/archive/tasks/README.md` (targeted lookups)
- Current architecture and decision evolution: `.meta/project/architecture.md`
- Current security/trust requirements: `.meta/project/security.md`
- Product brief: `.meta/project/brief.md`
- Implementation context: `.meta/project/context.md`
- Standards and verified command catalog: `.meta/project/standards.md`
- Agent development guide: `.meta/project/development.md`
- Rust style: `.meta/project/rust-style.md`
- Sources and terms: `.meta/project/source-map.md`; `.meta/project/glossary.md`

## Hygiene

- Last general consistency/pruning pass: 2026-09-08
- Completed repository-changing tasks since that pass: 12 (including T-0120)
- General pass due: 2026-10-08 or after 10 changes; still due.
- T-0120 performs scoped migration consistency and pruning, not a new application or
  general project-freshness audit. Preserve the broader overdue maintenance action.
