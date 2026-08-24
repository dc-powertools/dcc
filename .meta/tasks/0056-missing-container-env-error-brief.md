# T-0056 Brief: Missing `containerEnv` Error

## Identity And Source

- Task ID: T-0056
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User request
- Source reference and date: Product-behavior correction, 2026-08-24
- Related tasks: T-0006, T-0051

## Goal

A `${containerEnv:VAR}` reference without a default fails clearly when `VAR` is missing,
and that strict behavior is documented as the intended product contract.

## Background

T-0051 adopted an absent-variable compatibility behavior that substitutes an empty
string. The product contract is now clarified: silently continuing when the referenced
container environment variable is missing is incorrect. Defaults remain the explicit
way for a profile author to make absence acceptable.

## Scope

In scope:

- Restore a contextual error for an absent container environment variable referenced
  without a default.
- Preserve explicit default syntax as the supported fallback for an absent variable.
- Define, test, and document the behavior of a present-but-empty value separately from
  an absent key so the two cases are not accidentally conflated.
- Apply the contract consistently across state paths, lifecycle commands, and every
  other `${containerEnv:...}` consumer while retaining consumer-specific validation.
- Update public documentation, project architecture/source guidance, and supersede or
  amend decision 0005 so no accepted record claims missing values become empty.

Out of scope:

- Changing unrelated `${localEnv:...}` substitution rules.
- Bypassing post-substitution path or command validation.

## Acceptance Criteria

- [ ] A missing variable referenced as `${containerEnv:VAR}` produces a clear error
  naming the variable and relevant configuration context.
- [ ] A missing variable with an explicit default resolves to that default.
- [ ] Present empty and present non-empty values have intentional, documented tests.
- [ ] State-path, lifecycle, and other callers enforce the same missing-value contract.
- [ ] Public/project documentation and decision records no longer describe silent empty
  substitution for a missing value.

## Verification Plan

- Automated checks: focused substitution and consumer tests, CLI/config integration
  tests, full tests, lint, format, and build.
- Manual check: search documentation and decisions for stale absent-to-empty claims.

## Done When

Missing `${containerEnv:VAR}` references fail unless an explicit default is present,
the edge cases are named in tests, and the intended strict contract is documented.
