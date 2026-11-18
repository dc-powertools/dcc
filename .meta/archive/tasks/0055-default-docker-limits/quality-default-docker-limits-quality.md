# T-0055 Quality Record

- Date: 2026-08-24
- Change: Restore implicit 4 GiB memory and 2 CPU limits on runtime container
  creation while retaining independent explicit overrides.
- Route: Quick change
- Risk: Medium
- Owner or reviewer: T-0055 isolated implementer

## Scope And Criteria

- User-visible outcome: `dcc run`, `dcc exec`, `dcc attach`, and `dcc start` create
  containers with `--memory 4g --cpus 2` unless the corresponding CLI option is
  overridden.
- In scope: runtime CLI defaults, final Docker argv, CLI help, user documentation,
  architecture, cross-layer tests, and a superseding annotation on T-0050.
- Non-goals: Docker build resource flags, host-size-based selection, or changing an
  already-running container's resource configuration.

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Omitted options produce both defaults on every applicable creation entry point. | Argv-recording fake Docker matrix for `start`, `exec`, `attach`, and named `run`. | Every command emitted exactly one `--memory 4g` and one `--cpus 2`. | Pass |
| Either explicit option overrides only its own resource. | Fake Docker cases for memory-only, CPU-only, and both-explicit invocation. | `768m/2`, `4g/1.25`, and `768m/1.25` reached Docker unchanged. | Pass |
| Resource flags precede the image. | Locate the image immediately before the supervisor `--mode` argument and compare indexes. | Memory and CPU flag indexes preceded the image in every tested invocation. | Pass |
| CLI help and documentation state the defaults. | CLI integration help matrix plus documentation review. | All four help pages show `[default: 4g]` and `[default: 2]`; the user guide and architecture state the implicit contract. | Pass |
| The obsolete T-0050 unspecified-value conclusion remains historical and is explicitly superseded. | Task-record review. | T-0050's brief retains its original criteria and adds a T-0055 superseding correction. | Pass |

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | The user and T-0055 brief specify exact defaults, independent overrides, affected entry points, and non-goals. |
| Architecture and project context | Yes | All four public runtime commands pass resource values into one `RuntimePlan`; Docker build and short-lived internal containers use separate paths. |
| Data, security, and permissions | Yes | The change only restores bounded Docker resource flags and does not alter mounts, privileges, persisted data, or protocols. |
| Verification and rollback | Yes | The fake-Docker seam observes final argv without external state; reverting the task commit restores the prior optional behavior. |

Readiness verdict: **Ready**.

## Verification Results

| Required? | Check Or Method | Observed Result | Status | Unblocking Condition If Not Run |
| --- | --- | --- | --- | --- |
| Yes | Counterfactual default-resource boundary test | Failed before implementation because the recorded run call had no `--memory` pair. | Pass | N/A |
| Yes | `cargo test --test docker_boundary resource` | 3 passed, including all creation paths and independent overrides. | Pass | N/A |
| Yes | Focused CLI help test | 1 passed across `run`, `exec`, `start`, and `attach`. | Pass | N/A |
| Yes | `cargo check` | Passed. | Pass | N/A |
| Yes | `cargo clippy -- -D warnings` | Passed with warnings denied. | Pass | N/A |
| Yes | `cargo test` | Passed: 504 unit; 31 runnable CLI with 3 ignored; 9 config; 9 fake-Docker; 9 Feature CLI; 32 Docker smokes compiled and ignored. | Pass | N/A |
| Yes | `cargo build` | Passed. | Pass | N/A |
| Yes | `cargo fmt --check` | Passed. | Pass | N/A |
| Yes | `git diff --check` and scoped diff review | Passed; no unrelated implementation changes found. | Pass | N/A |

- Criteria or methods amended after implementation began, with reason and impact: the
  default boundary regression was expanded from `start` to all four public creation
  entry points, and help output received an automated matrix because the task explicitly
  makes the defaults public CLI behavior.
- Counterfactual evidence for new regression or behavior tests: the default-resource
  matrix failed against the pre-fix implementation at its first missing `--memory`
  assertion, then passed after restoring defaults.
- Flaky result and disposition: none.

## Review Findings

| Severity | File Or Area | Finding | Required Fix | Status |
| --- | --- | --- | --- | --- |
| N/A | Runtime creation call sites | All four resource-bearing CLI variants converge on `RuntimePlan`; no additional applicable creation path was found. | None | Resolved |

## Consistency

| Canonical Source | Expected | Actual | Required Update |
| --- | --- | --- | --- |
| User request and T-0055 brief | Implicit 4g/2 with independent overrides. | Implementation and boundary tests match. | None |
| T-0050 history | Preserve the completed record but identify its corrected conclusion. | Original criteria remain, with a named superseding T-0055 annotation. | None |
| Public/project docs | State the defaults and affected commands. | CLI help, user guide, and architecture agree. | None |
| Tests | Fail if defaults are omitted or misplaced. | Final argv matrix asserts values, uniqueness, and ordering. | None |

## Batch And Residual Risk

- Large-diff split trigger hit: No.
- Risk not resolved by passing checks: live Docker resource enforcement was not run in
  this environment. The fake-Docker boundary proves the exact documented arguments sent
  to Docker; Docker itself owns their enforcement semantics.

## Completion

- Required checks all passed: Yes
- Status: Done
- Exact incomplete condition, if not Done: None
- Next action: Integrate the isolated task commit and close T-0055 in the root-owned
  catalog/cursor.
