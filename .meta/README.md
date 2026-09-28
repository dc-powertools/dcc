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
| 2026-09-28 | Examined relay placements and recorded the user's selection of a proxy managed by the existing development-container supervisor, excluding separate containers (T-0095). | `.meta/decisions/0008-relay-in-development-container.md` |
| 2026-09-28 | Diagnosed concurrent foreground sessions colliding over per-process port-forwarding listeners during attach (T-0094); source inspection only. | `.meta/tasks/0094-attach-port-collision.md` |
| 2026-09-28 | Bumped the patch version from 0.1.8 to 0.1.9 after Feature validation (T-0093). | `.meta/tasks/README.md#tasks` |
| 2026-09-28 | Added offline Feature publication validation with a pinned schema, upstream-only mode, and shared project/Feature mount parsing that preserves string readonly flags (T-0092). | `.meta/quality/0092-feature-validation.md` |
| 2026-09-28 | Bumped the project patch version from 0.1.7 to 0.1.8 in a separate local commit after the bootstrap fix (T-0091). | `.meta/tasks/README.md#tasks` |
| 2026-09-28 | Fixed profile bootstrap in new repositories, with Git-root/current-directory fallback and side-effect-free dry-run (T-0090). | `.meta/tasks/README.md#tasks` |
| 2026-09-09 | Bumped the project patch version from 0.1.6 to 0.1.7 without triggering release automation (T-0089). | `.meta/tasks/README.md#tasks` |
| 2026-09-09 | Made `ghcr.io/dc-powertools/features` the built-in repository for short Feature names while retaining project overrides (T-0088). | `.meta/tasks/0088-built-in-default-feature-repository-notes.md` |
| 2026-09-08 | Implemented declaration-scoped configurable default Feature repository resolution and canonical shorthand editing (T-0087). | `.meta/tasks/0087-default-feature-repository-notes.md` |
| 2026-09-08 | Designed declaration-scoped configurable default Feature repository resolution with contained tool footprint (T-0086). | `.meta/tasks/0086-default-feature-repository-design.md` |
| 2026-09-08 | Implemented trustworthy running-container status in `dcc profile list`, including `[running]` text and tri-state JSON (T-0085). | `.meta/tasks/0085-profile-running-indicator-notes.md` |
| 2026-09-08 | Designed a trustworthy, batched running-container indicator for `dcc profile list` (T-0084). | `.meta/tasks/0084-profile-running-indicator-design.md` |
| 2026-09-08 | Removed and separately committed the obsolete `old` devcontainer profile and lockfile (T-0083). | `.meta/tasks/README.md#tasks` |
| 2026-09-08 | Added exclusive profile bootstrapping from an image, Dockerfile, or validated parent profile (T-0082). | `.meta/tasks/README.md#tasks` |
| 2026-08-25 | Completed registry-scoped custom-CA support and verified its TLS OCI package-to-image path through marker execution and exact cleanup (T-0070). | `.meta/quality/0073-tls-oci-docker-smoke-quality.md` |
| 2026-08-25 | Remediated CI token exposure with read-only workflow permissions and non-persisted checkout credentials (T-0081). | `.meta/tasks/README.md#tasks` |

## Documentation Map

- Reusable framework: `.meta/meta/README.md`
- Task catalog: `.meta/tasks/README.md`
- State seeding decision: `.meta/decisions/0001-state-seeding-from-image.md`
- UID remap decision: `.meta/decisions/0002-update-remote-user-uid-in-build-stage.md`
- Remove image fast path decision: `.meta/decisions/0003-remove-image-fast-path.md`
- Supervisor delivery model decision: `.meta/decisions/0004-embed-supervisor-in-image.md`
- Relay placement decision: `.meta/decisions/0008-relay-in-development-container.md`
- Running configuration drift policy: `.meta/decisions/0009-warn-and-defer-running-config-changes.md`
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

- Last consistency and pruning pass: 2026-09-08
- Completed repository-changing tasks since that pass: 8
- Next pass due: 2026-10-08 or after 10 completed repository-changing tasks, whichever
  occurs first.
- Incomplete maintenance actions: None.
