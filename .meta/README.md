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
| 2026-08-17 | Reorganized public docs again (T-0040): moved the broad guide to `docs/index.md`, made `docs/features.md` focus on devcontainer Feature packages, linked it to the official Features reference, and updated cross-links/ownership notes. | `.meta/tasks/README.md#tasks` |
| 2026-08-17 | Reorganized public docs (T-0039): shortened the main README into an end-user overview, added `docs/features.md` for detailed feature/configuration/runtime guidance, added `docs/development.md` for maintainer and release guidance, linked both from README, and corrected stale baked-supervisor wording. | `.meta/tasks/README.md#tasks` |
| 2026-08-17 | Updated the local commit trailer rule (T-0035) so `Co-Authored-By` names the actual model and coding harness, with Codex GPT-5 as this session's example. | `.meta/tasks/README.md#tasks` |
| 2026-08-17 | Fixed the latest CI Docker smoke failure (T-0038): fast one-shot commands now persist a supervisor arrival marker so PID 1 drains immediately even when polling misses the active-record lifetime. | `.meta/tasks/README.md#tasks` |
| 2026-08-17 | Fixed the latest CI Docker smoke failure (T-0037): one-shot foreground executions now wait for the label-based running-container query to go empty only when the invocation created the container, matching reuse detection and avoiding durable-container delays. | `.meta/tasks/README.md#tasks` |

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

- Last consistency and pruning pass: 2026-08-13
- Completed repository-changing tasks since that pass: 7
- Next pass due: 2026-09-12 or after 10 completed repository-changing tasks, whichever
  occurs first.
