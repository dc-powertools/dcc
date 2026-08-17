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
- Completed repository-changing tasks since that pass: 0
- Next pass due: 2026-09-12 or after 10 completed repository-changing tasks, whichever
  occurs first.
