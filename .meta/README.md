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
| 2026-08-25 | Bumped the project patch version from 0.1.5 to 0.1.6 locally without triggering release automation (T-0078). | `.meta/tasks/README.md#tasks` |
| 2026-08-25 | Restored fixed container-variable substitution for Feature-contributed state while preserving deferred environment and path-safety validation (T-0077). | `.meta/tasks/README.md#tasks` |
| 2026-08-25 | Bumped the project patch version from 0.1.4 to 0.1.5 locally without triggering release automation (T-0074). | `.meta/tasks/README.md#tasks` |
| 2026-08-25 | Added deterministic Docker-free `dcc profile list` discovery with stable, filename-safe text and structured JSON output (T-0057). | `.meta/quality/0057-profile-list-quality.md` |
| 2026-08-25 | Restored OCI Feature compatibility for safe archive-root directory entries while retaining archive traversal and entry-type protections (T-0069). | `.meta/tasks/README.md#tasks` |

## Documentation Map

- Reusable framework: `.meta/meta/README.md`
- Task catalog: `.meta/tasks/README.md`
- State seeding decision: `.meta/decisions/0001-state-seeding-from-image.md`
- UID remap decision: `.meta/decisions/0002-update-remote-user-uid-in-build-stage.md`
- Remove image fast path decision: `.meta/decisions/0003-remove-image-fast-path.md`
- Supervisor delivery model decision: `.meta/decisions/0004-embed-supervisor-in-image.md`
- Current `containerEnv` substitution decision: `.meta/decisions/0006-require-missing-container-env-default.md`
- Superseded `containerEnv` compatibility decision: `.meta/decisions/0005-container-env-substitution.md`
- Rewrite quality record: `.meta/quality/0004-dcc-rewrite-quality.md`
- UID remap quality record: `.meta/quality/0026-update-remote-user-uid-quality.md`
- Runtime threat model: `.meta/threat-models/0004-dcc-runtime.md`
- CI ref and fixture threat model: `.meta/threat-models/0062-ci-ref-and-fixture.md`
- Release CI reuse threat model: `.meta/threat-models/0063-release-ci-reuse.md`
- Product brief: `.meta/project/brief.md`
- Implementation context: `.meta/project/context.md`
- Project standards and command catalog: `.meta/project/standards.md`
- Detailed architecture: `.meta/project/architecture.md`
- Detailed development guide: `.meta/project/development.md`
- Detailed Rust style guide: `.meta/project/rust-style.md`
- Source map: `.meta/project/source-map.md`
- Glossary: `.meta/project/glossary.md`

## Hygiene

- Last consistency and pruning pass: 2026-08-24
- Completed repository-changing tasks since that pass: 7
- Next pass due: 2026-09-23 or after 10 completed repository-changing tasks, whichever
  occurs first.
- Incomplete maintenance actions: None.
