# T-0051 Brief: Undefined `containerEnv` Contract

## Identity And Source

- Task ID: T-0051
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

Undefined and empty `${containerEnv:VAR}` substitutions have one intentional contract
across implementation, public documentation, project architecture, and tests.

## Background

`src/config/vars.rs` and its tests treat an undefined or empty value without a default as
an error. `docs/index.md` says an undefined variable resolves to an empty string. The
test currently fossilizes one side of a product/compatibility conflict rather than
demonstrating an agreed outcome.

## Decision Questions

1. Should undefined values resolve to an empty string for devcontainer compatibility,
   or fail to prevent silently malformed paths and commands?
2. Is an explicitly empty environment variable distinguishable from an absent variable?
3. How do `${containerEnv:VAR:-default}` and fields with additional validation interact
   with the chosen behavior?
4. If `dcc` intentionally deviates from the upstream behavior, where is that deviation
   disclosed to users?

## Scope

In scope:

- Decide the questions above using public compatibility, error discoverability, and
  safety consequences.
- Record a decision if retaining an intentional compatibility deviation.
- Align substitution behavior, tests, `docs/index.md`, and project architecture with the
  chosen contract; reroute or split implementation after the decision if appropriate.
- Cover absent, explicitly empty, non-empty, defaulted, and post-substitution validation
  cases.

Out of scope:

- Changing unrelated `${localEnv:...}` rules.
- Silent documentation-only reconciliation without deciding actual product behavior.

## Acceptance Criteria

- [ ] One written contract answers absent versus empty and default behavior.
- [ ] Implementation and public/project documentation agree with it.
- [ ] Tests are named for the chosen user outcome, not the current branch behavior.
- [ ] State-path and lifecycle consumers retain their own post-substitution validation.
- [ ] Counterfactual coverage distinguishes empty substitution from default use and
  malformed resolved values.

## Workflow Route Rationale

- Cataloged route and risk: Decide / Medium.
- Why this route: Both current behaviors are plausible; choosing one changes public
  compatibility and error semantics.
- Why this risk gate: Silent empty substitution can corrupt commands or paths, while a
  hard error can reject otherwise compatible profiles.
- Escalation trigger: Create an implementation child if the decision requires a broad
  migration or deprecation path.

## Done When

A profile author can predict undefined, empty, and defaulted `containerEnv` behavior
from the docs, and the suite enforces exactly that contract.

## Subsequent Product Correction

T-0051 completed the upstream-compatible contract described above. On 2026-08-24 the
product owner clarified that dcc's intended behavior is stricter: an absent
`${containerEnv:VAR}` without an explicit default must be an error. T-0056 and decision
0006 supersede that behavior without changing this record of what T-0051 implemented.
