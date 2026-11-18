# Current Tasks

Discovery only. Each linked task README owns authority, revision, lifecycle,
dependencies, approvals/blockers, next action and results. Global scheduling and the
primary-task pointer live in `.meta/README.md`.

| Task | Summary / Keywords |
| --- | --- |
| [<id>](<id>-<slug>/README.md) | Short outcome and optional search keywords |

Historical tasks at `.meta/archive/tasks/README.md` are a separate targeted lookup.
Link to that index using a relative path from the instantiated index. Create
that index only when terminal bundles exist. Use `# Archived Tasks` for its heading.
