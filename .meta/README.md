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
| 2026-08-24 | Implemented macOS `updateRemoteUserUID` mapping (T-0054) while preserving Linux behavior and explicit unsupported-host no-ops. | `.meta/quality/0054-macos-uid-remap-quality.md` |
| 2026-08-24 | Completed T-0043 test-quality corrections: all ten children now protect stable behavior, compatibility, security, and cross-layer contracts. | `.meta/quality/0043-test-quality-corrections-quality.md` |
| 2026-08-24 | Removed `dcc build --update` (T-0042): `--no-cache` now also passes Docker `--pull` when building from an upstream base image. | `.meta/tasks/README.md#tasks` |
| 2026-08-24 | Removed lockfile implementation (T-0041): deleted `devcontainer.lock` read/write behavior and locked Feature digest plumbing, while preserving OCI blob digest verification for fetched Features. | `.meta/tasks/README.md#tasks` |
| 2026-08-17 | Reorganized public docs again (T-0040): moved the broad guide to `docs/index.md`, made `docs/features.md` focus on devcontainer Feature packages, linked it to the official Features reference, and updated cross-links/ownership notes. | `.meta/tasks/README.md#tasks` |
| 2026-08-17 | Reorganized public docs (T-0039): shortened the main README into an end-user overview, added `docs/features.md` for detailed feature/configuration/runtime guidance, added `docs/development.md` for maintainer and release guidance, linked both from README, and corrected stale baked-supervisor wording. | `.meta/tasks/README.md#tasks` |

## Documentation Map

- Reusable framework: `.meta/meta/README.md`
- Task catalog: `.meta/tasks/README.md`
- State seeding decision: `.meta/decisions/0001-state-seeding-from-image.md`
- UID remap decision: `.meta/decisions/0002-update-remote-user-uid-in-build-stage.md`
- Remove image fast path decision: `.meta/decisions/0003-remove-image-fast-path.md`
- Supervisor delivery model decision: `.meta/decisions/0004-embed-supervisor-in-image.md`
- `containerEnv` substitution decision: `.meta/decisions/0005-container-env-substitution.md`
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

- Last consistency and pruning pass: 2026-08-24
- Completed repository-changing tasks since that pass: 1
- Next pass due: 2026-09-23 or after 10 completed repository-changing tasks, whichever
  occurs first.
