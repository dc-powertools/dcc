# T-0087 Default Feature Repository Quality Record

- Date: 2026-09-08
- Change: declaration-scoped configurable default Feature repository implementation
- Route: Initiative
- Risk: Medium
- Owner or reviewer: T-0087 primary session, focused design and diff review

## Scope And Criteria

- User-visible outcome: project profiles can declare short Feature names that resolve
  beneath one checked-in default OCI repository.
- In scope: config parsing/inheritance, Feature reference identity, dependencies, OCI
  parsing, `dcc feature`, tests, and public/architecture documentation.
- Non-goals: new commands, global/environment configuration, repository search,
  network fallback, digest support, persistence, permissions, or dependencies.

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Valid project-local repository prefix | Parser matrix and strict-config tests | Authority ports/IPv6 and OCI separators accepted; URLs, tags, digests, credentials, malformed paths, variables, and whitespace rejected offline | Pass |
| Short and untagged identities are canonical | Reference, config, and OCI parser tests | `sudo` expands below the default with `:latest`; explicit omitted tags also become `latest`; local paths remain literal | Pass |
| Inheritance preserves declaration provenance | Parent/child resolver tests | Child inheritance and override work without rebinding parent shorthand; canonical child options win in parent order | Pass |
| Dependencies and metadata retain their trust boundary | Dependency and metadata tests | Explicit dependency aliases deduplicate; short dependencies and Feature-controlled defaults are rejected | Pass |
| Feature editing is identity-aware and selected-file-only | Unit and CLI integration matrix | Requested spelling is written, aliases match, inherited entries stay untouched, and exact removal repairs missing/invalid defaults | Pass |
| Footprint remains contained | Manifest, CLI, config-source, and diff review | No crate, command, override source, artifact, fallback request, privilege, or external action added | Pass |
| Documentation matches behavior | README, user guide, Feature guide, and architecture read-through | Validation, implicit tag, inheritance, portability, editing, and explicit dependency behavior documented | Pass |

## Verification Results

| Required? | Check Or Method | Observed Result | Status | Unblocking Condition If Not Run |
| --- | --- | --- | --- | --- |
| Yes | `cargo fmt --check` | Passed with no diff | Pass | |
| Yes | `cargo check` | Passed for `dcc v0.1.6` | Pass | |
| Yes | `cargo clippy --all-targets -- -D warnings` | Passed with warnings denied | Pass | |
| Yes | Focused config/reference/editor/dependency tests | All focused tests passed, including 15 Feature CLI integration cases | Pass | |
| Yes | `cargo test` | 587 unit and 93 runnable integration tests passed; 38 environment-bound tests remained intentionally ignored | Pass | |
| Yes | `cargo build` | Passed for the dev profile | Pass | |
| Yes | `git diff --check` and focused diff/design review | No whitespace, scope, secret-output, or remaining design finding | Pass | |

- Criteria or methods amended after implementation began, with reason and impact:
  OCI repository separators, inherited editor defaults, invalid-default repair, and
  parent-file non-editing received explicit tests during review; these strengthened the
  accepted contract without expanding it.
- Counterfactual evidence for new regression or behavior tests: the omitted-tag OCI
  test failed against the prior parser with `feature reference must include a tag`.
  The prior `load_raw` returned or merged raw keys without normalization, so the new
  canonical parent/child assertions also contradict the inspected baseline directly.
- Flaky result and disposition: None.

## Review Findings

| Severity | File Or Area | Finding | Required Fix | Status |
| --- | --- | --- | --- | --- |
| Medium | OCI repository grammar | Initial validation rejected valid double-underscore and repeated-dash separators | Implement the standard component separator forms and add positive/negative cases | Resolved |
| Medium | `dcc feature` repair behavior | Ordinary typed config validation would prevent exact removal when the default itself was invalid | Permit a tightly gated exact-remove-only path for default/shorthand validation failures | Resolved |
| Low | Architecture | Dependency queue documentation still named the retired set shape | Describe the canonical-reference options map actually used | Resolved |

## Consistency

| Canonical Source | Expected | Actual | Required Update |
| --- | --- | --- | --- |
| User request and T-0086 | Configurable default with escalation only for significant footprint | Approved contained design implemented with no footprint expansion | None |
| Project standards | Private Rust modules, no new dependency, full local gates | Shared crate-private model and existing crates only; all gates pass | None |
| Tests and docs | Declaration provenance and one canonical identity everywhere | Resolver, dependency, OCI, editor, and docs align | None |
| State and catalog | One completed T-0087 task and clean primary cursor | Completion records updated; hygiene pass reset the task counter | None |

## Batch And Residual Risk

- Large-diff split trigger hit: Yes.
- If kept together, why: config normalization, dependency/OCI identity, and the editor
  must share one grammar to prevent source rebinding or duplicate installation. The
  accepted T-0086 design defined them as one coherent contract, and each boundary has
  focused tests before the full gate.
- Risk not resolved by passing checks: implicit `latest` remains mutable by design,
  matching upstream behavior. Live registry/Docker execution was not required because
  expansion ends at the already-covered OCI request boundary and adds no transport or
  build behavior.

## Completion

- Required checks all passed: Yes.
- Status: Done
- Exact incomplete condition, if not Done: None.
- Next action: Commit the task-scoped implementation and records locally.
