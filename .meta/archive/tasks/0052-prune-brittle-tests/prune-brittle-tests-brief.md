# T-0052 Brief: Prune Brittle And Redundant Tests

## Identity And Source

- Task ID: T-0052
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

The suite contains no known assertions whose only purpose is preserving retired names,
exact internal formatting, duplicate examples, or historical cleanup structure.

## Background

The audit identified tests that assert supervisor variable names and exact sleep text,
an exact generated-asset count and absence of an old hook directory, the absence of a
retired dry-run phrase, Dockerfile line placement rather than semantic order, and
duplicated workspace/cache/profile examples. Such tests create maintenance cost while
providing little regression confidence.

## Candidate Inventory

- Supervisor tests for exact `REAPER_SECS`, `arrived`, retired
  `STARTUP_GRACE`/`PRIMED` tokens, and exact `sleep 0.2` text when real-shell behavior is
  already covered.
- `generated_assets_returns_only_supervisor_scripts` exact length and retired-path
  negative assertion.
- `stop_dry_run_no_longer_reports_runtime_state_clearing` retired phrase assertion.
- The root-remap Dockerfile test whose name says a plan is present while passing `None`,
  exact label placement tests, and install-`nc` ordering tested without any Features.
- Duplicate workspace-root, cache absolute-path, and profile format examples already
  subsumed by stronger exact-result tests.

## Scope

In scope:

- Classify each candidate as delete, rewrite around a stable invariant, or retain with a
  documented load-bearing reason.
- Preserve real shell execution, lifecycle status, required generated assets, version
  labeling, and valid Dockerfile ordering through stronger existing or replacement
  tests.
- Rename misleading tests whose setup does not match their title.

Out of scope:

- Reducing test count as a goal.
- Deleting slow or inconvenient tests that uniquely protect a meaningful outcome.
- Adding the broader new property matrices owned by T-0053.

## Acceptance Criteria

- [ ] Every named candidate is classified and handled.
- [ ] No retained assertion depends only on a retired symbol, absent historical phrase,
  exact collection length, or incidental whitespace/line location.
- [ ] Required supervisor behavior, generated assets, Dockerfile semantics, and public
  dry-run output remain covered.
- [ ] Test names accurately describe setup and observed outcome.
- [ ] Diff review confirms deletion did not remove the only coverage of a stable
  behavior.

## Verification Plan

- Automated checks: focused affected test modules, full tests, lint, format, and build.
- Manual checks: map each deleted assertion to retained/replacement coverage and review
  for accidental behavior weakening.

## Done When

The known low-value tests are gone or express stable contracts, with no meaningful
coverage silently lost.
