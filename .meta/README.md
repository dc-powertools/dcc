# Project State

Read this file at the start of every session. It is the bounded project-wide cursor and
documentation index, not a task list or history log.

## Task Cursor

- Task catalog: `.meta/tasks/README.md`
- Primary task: None
- Primary details: None

The task catalog owns outcomes, status, dependencies, task-specific approvals or
blockers, next actions, detail links, and results. Do not copy them here.

## Global Parked Approvals

| ID | Gated Action | Status | Affected Tasks | Decision Record |
| --- | --- | --- | --- | --- |
| None | | | | |

## Known Global Dead Ends

- Baking startup hook scripts (`postStartCommand`) into the image — blocked because
  `apply_substitutions` resolves `${localEnv:VAR}` in `lifecycle` from the invoking `dcc`
  process's environment, so hook text is only fully resolvable at run time. Baking would
  either freeze the builder's environment into the image or require a second substitution
  engine inside the container. See
  `.meta/decisions/0004-embed-supervisor-in-image.md` Q3.
- Baking a seed store into the image (extra build stage staging state tarballs) — rejected
  for image overhead; the image already holds the data at its natural path. See
  `.meta/decisions/0001-state-seeding-from-image.md`.
- Using `mv` instead of `cp` to relocate state data during build to control image size —
  ineffective, because additive layers never reclaim bytes from the layer that created the
  path, and it breaks standalone `docker run` use of the image. Same decision record.

## Recently Completed

| Date | Outcome | Durable Record |
| --- | --- | --- |
| 2026-08-13 | Relocated the framework documentation tree from `readme/` to `.meta/` (T-0033) and updated every internal link and reference, including root `AGENTS.md` and two Rust doc comments. The reusable core is now `.meta/meta/`. | `.meta/tasks/README.md#tasks` |
| 2026-08-13 | Baked the supervisor scripts into the image (T-0029), added a semver compatibility gate on `dcc.version` (T-0030), and removed vestigial build-prep hook assets (T-0031). | `.meta/decisions/0004-embed-supervisor-in-image.md` |
| 2026-08-13 | Decided the supervisor delivery model (T-0028): **bake the supervisor scripts into the image, keep `postStartCommand` hooks on the `rt` bind mount**, and gate compatibility on `dcc.version` semver (patch compatible; major/minor or missing label refuses). Hooks cannot be baked because `${localEnv:VAR}` in `postStartCommand` is only resolvable at run time. Implementation split to T-0029/T-0030. | `.meta/decisions/0004-embed-supervisor-in-image.md` |
| 2026-08-13 | Moved startup sequencing and runtime lifecycle hooks into the in-container supervisor (T-0025): `--mode`/`--expect-command`/`--start-hooks` entrypoint args, supervisor-run `postStartCommand` from host-generated scripts, single `bootstrap-status` file with per-waiter FIFO readiness handshake (no polling, no package dependency), `dcc-exec` register-then-wait, `arrived` variable + 10 s one-shot orphan reaper replacing the time-based grace; `postAttachCommand` stays host-side with cold-start `wait-ready`. | `.meta/tasks/README.md#tasks` |
| 2026-07-21 | Seeded declared `customizations.dcc.state` from the image on build (hydration container, `dcc.seed` label, `.dcc/<profile>.seed.json` ledger, `--reseed-state`, runtime guard). | `.meta/tasks/README.md#tasks` |
| 2026-07-21 | Guarded `customizations.dcc.state` against critical container paths (two-tier subtree/exact reserved-path guards at load and post-`containerEnv` resolution). | `.meta/tasks/README.md#tasks` |
| 2026-07-21 | Added `dcc feature --add`/`--remove` for profile Feature edits. | `.meta/tasks/README.md#tasks` |
| 2026-07-20 | Made `--debug` a global CLI flag and added debug output for build, stop, and id. | `.meta/tasks/README.md#tasks` |
| 2026-07-20 | Moved README command flag guidance into a dedicated `Global flags` subsection. | `.meta/tasks/README.md#tasks` |
| 2026-07-20 | Reorganized README lifecycle-hook documentation into a dedicated trigger table. | `.meta/tasks/README.md#tasks` |
| 2026-07-20 | Removed `initializeCommand` execution so the hook is only parsed with an unsupported warning. | `.meta/tasks/README.md#tasks` |
| 2026-07-20 | Updated user-facing README guidance to match the completed `dcc` rewrite behavior and current CLI design. | `.meta/tasks/README.md#tasks` |
| 2026-07-20 | Added expanded ignored Docker behavior tests for lifecycle phases, state persistence, durable/one-shot reuse, workspaceFolder, env substitution, and Feature metadata. | `.meta/tasks/README.md#tasks` |
| 2026-07-20 | Added `--dry-run` and `--format json` support for Docker-free CLI validation and converted fragile CLI acceptance checks to dry-run assertions. | `.meta/tasks/README.md#tasks` |
| 2026-07-20 | Added a GitHub-hosted Docker smoke test workflow job while keeping Docker tests ignored for local development-container test runs. | `.meta/tasks/README.md#tasks` |
| 2026-07-15 | Installed validation tooling and completed official Dev Container CLI config validation for the `dcc` rewrite. | `.meta/quality/0004-dcc-rewrite-quality.md` |
| 2026-07-14 | Completed final `dcc` compatibility, validation records, and parent rewrite closure. | `.meta/tasks/README.md#tasks` |
| 2026-07-14 | Implemented durable runtime lifecycle commands and one-shot bookkeeping. | `.meta/tasks/README.md#tasks` |
| 2026-07-14 | Implemented official build-source support, generated controller/hook assets, and build preparation. | `.meta/tasks/README.md#tasks` |
| 2026-07-14 | Implemented Feature metadata compatibility for commands, state, unsupported properties, and unsafe runtime gating. | `.meta/tasks/README.md#tasks` |
| 2026-07-14 | Implemented validated `customizations.dcc.state` path handling and profile-local state mount planning. | `.meta/tasks/README.md#tasks` |
| 2026-07-14 | Implemented schema-compatible `customizations.dcc` config parsing and merge behavior. | `.meta/tasks/README.md#tasks` |
| 2026-07-14 | Migrated development, style, and architecture docs into framework-owned project files. | `.meta/tasks/README.md#tasks` |
| 2026-07-14 | Merged backup agent guidance into framework state and removed `AGENTS.bak.md`. | `.meta/tasks/README.md#tasks` |
| 2026-07-14 | Initialized the framework project cursor, task catalog, and project memory. | `.meta/tasks/README.md#tasks` |

## Documentation Map

- Reusable framework: `.meta/meta/README.md`
- Task catalog: `.meta/tasks/README.md`
- State seeding decision: `.meta/decisions/0001-state-seeding-from-image.md`
- UID remap decision: `.meta/decisions/0002-update-remote-user-uid-in-build-stage.md`
- Remove image fast path decision: `.meta/decisions/0003-remove-image-fast-path.md`
- Supervisor delivery model decision: `.meta/decisions/0004-embed-supervisor-in-image.md`
- Rewrite quality record: `.meta/quality/0004-dcc-rewrite-quality.md`
- UID remap quality record: `.meta/quality/0026-update-remote-user-uid-quality.md`
- Runtime threat model: `.meta/threat-models/0004-dcc-runtime.md`
- Product brief: `.meta/project/brief.md`
- Implementation context: `.meta/project/context.md`
- Project standards and command catalog: `.meta/project/standards.md`
- Detailed architecture: `.meta/project/architecture.md`
- Detailed development guide: `.meta/project/development.md`
- Detailed Rust style guide: `.meta/project/rust-style.md`
- Source map: `.meta/project/source-map.md`
- Glossary: `.meta/project/glossary.md`

## Hygiene

- Last consistency and pruning pass: 2026-07-14
- Completed repository-changing tasks since that pass: 24
- Next pass due: **Overdue** (was due 2026-08-13 or after 10 completed repository-changing
  tasks; both thresholds are passed). Run a consistency and pruning pass before or
  alongside the next task.
