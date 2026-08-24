# T-0053 Brief: Property And Behavior-Matrix Tests

## Identity And Source

- Task ID: T-0053
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

High-combinatorial-risk helpers and editing workflows are tested through meaningful
invariants and behavior matrices rather than a handful of narrow examples or misleading
partial properties.

## Background

The current merge property generates only an optional image and asserts only that field.
Shell quoting and Dockerfile generation rely mainly on exact example strings. Seed
digest tests check stability/shape but not whether meaningful input changes alter the
digest. Feature command integration covers the happy path but not JSONC preservation,
combined add/remove, duplicate/no-op, and missing-removal behavior.

## Scope

In scope:

- Config merge generators spanning optional scalar fields, maps, lists, Features,
  lifecycle commands, and `customizations.dcc`, with explicit parent/child precedence
  and identity laws.
- Shell-quote properties verified by executing safe generated arguments through
  `/bin/sh`, including whitespace, quotes, dollar signs, newlines, and empty strings.
- Dockerfile invariants such as required stage/label/assets and package installation
  occurring after Feature install steps, without pinning incidental whitespace.
- Seed digest sensitivity to content, path/order normalization, symlink target, and mode
  where each is part of the intended invalidation contract.
- Feature command behavior matrix for JSONC input, add and remove together, duplicate
  additions, no-op summaries, and missing removals, preserving unrelated configuration.

Out of scope:

- Randomized live Docker builds.
- Snapshotting whole generated files when smaller semantic assertions suffice.
- Inventing digest sensitivity for metadata intentionally excluded from the seed
  contract; record that choice instead.

## Acceptance Criteria

- [ ] Merge properties exercise all material field families and fail when precedence or
  identity is deliberately broken.
- [ ] Shell-quote properties round-trip arbitrary bounded fixture arguments through the
  real supported shell.
- [ ] Dockerfile tests include at least one Feature and assert semantic ordering needed
  for correctness.
- [ ] Seed digest tests distinguish stability from sensitivity and document which file
  attributes affect invalidation.
- [ ] Feature editing tests cover the full matrix above and assert unrelated config is
  preserved.
- [ ] Generators have bounded sizes and deterministic failure reproduction.

## Workflow Route Rationale

- Cataloged route and risk: Initiative / Medium.
- Why this route: The task spans several independent combinatorial test surfaces but has
  one coherent outcome: replace weak examples with high-leverage invariants.
- Why this risk gate: Incorrect generators or asserted algebra can institutionalize a
  false contract across broad input space.
- Escalation trigger: Split a subsystem if its intended invariant is ambiguous or the
  implementation change would cross a separate product boundary.

## Verification Plan

- Automated checks: focused property suites with deterministic regression seeds, Feature
  command integration tests, full tests, lint, format, and build.
- Counterfactual checks: safe mutations/negative controls for merge precedence, quote
  escaping, Dockerfile ordering, digest inputs, and no-op editing.
- Manual checks: review each invariant against public/project contracts rather than the
  current implementation.

## Done When

The named combinatorial surfaces have bounded, reproducible tests that fail for
meaningful regressions and permit harmless refactoring.
