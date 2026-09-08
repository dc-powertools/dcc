# T-0087: Default Feature Repository Implementation Notes

## Resumption

- Goal: Implement the accepted T-0086 design without expanding its approved footprint.
- Design owner: `.meta/tasks/0086-default-feature-repository-design.md`
- Current slice: Complete.
- Required verification: focused config, Feature resolver/OCI, and Feature editor tests;
  `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`;
  `cargo test`; `cargo build`; documentation and diff review.
- External-state boundary: no live registry or Docker operation is required; use existing
  local and loopback fixtures.

## Progress

- 2026-09-08: Implementation approved by the product owner; task activated from the
  completed T-0086 design.
- 2026-09-08: Added the shared validated repository/reference model, including strict
  OCI prefix and shorthand grammar, canonical registry authority, and implicit
  `latest`. The omitted-tag test failed against the previous parser before the change.
- 2026-09-08: Normalized each file's Feature declarations before merging, preserving
  parent/child default provenance, canonical child overrides, order, and same-file
  collision errors.
- 2026-09-08: Reused canonical identities in OCI download and Feature `dependsOn`
  resolution; project shorthand remains forbidden in dependency metadata and Feature
  metadata cannot configure the project default source.
- 2026-09-08: Made `dcc feature` compare direct entries canonically while preserving
  requested spelling, inherited defaults, selected-file ownership, dry-run behavior,
  and exact repair removal for absent or invalid defaults.
- 2026-09-08: Updated README, user guide, Feature guide, and architecture. No command,
  dependency, global config source, persistent artifact, privilege, or network fallback
  was added, so the approved contained footprint remained intact.
- 2026-09-08: `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test` (587 unit and 93 runnable
  integration tests; 38 intentionally ignored), `cargo build`, and `git diff --check`
  passed. Focused correctness, user-impact, safety, maintainability, test, complexity,
  documentation, and design-consistency review found no remaining issue.
- 2026-09-08: The scheduled ten-change consistency/pruning pass validated cursor and
  catalog state, links, task IDs/dependencies, active-task cardinality, parked
  approvals, command ownership, and artifact budgets. No stale guidance, due project
  pilot, duplicate owner, or pruning action remained; the hygiene counter was reset.
