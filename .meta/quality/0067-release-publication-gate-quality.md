# T-0067 Release Publication Gate Quality

- Date: 2026-08-25
- Change: Ensure `Create Release` runs after successful release builds when reusable CI is intentionally skipped.
- Route: Quick change
- Risk: High
- Owner or reviewer: Root Orchestrator

## Scope And Criteria

- User-visible outcome: Automatic releases publish after exact-commit CI and every release build succeeds.
- In scope: The final publication job condition, static workflow-contract coverage, release threat model, and failed-release incident record.
- Non-goals: Triggering a release, pushing the existing tag, changing release permissions, or changing direct-tag CI behavior.

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Trusted CI reuse allows all release builds and publication after successful builds. | Static workflow contract check plus expression review. | `release` explicitly overrides skipped-ancestor propagation and requires `needs.build.result == 'success'`; the contract check passed. | Pass |
| Failed or cancelled release builds cannot publish. | Inspect the explicit publication condition and its negative contract assertions. | Publication requires `!cancelled()` and a successful aggregated build result. | Pass |
| Direct tag pushes still require successful release CI. | Static workflow contract check. | The conditional release CI and result-aware build gate remain required by the passing contract. | Pass |
| Workflow syntax and action contracts remain valid. | `actionlint .github/workflows/*.yml`. | Passed with `actionlint 1.7.12`. | Pass |

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | The run and GitHub API identify only `release / Create Release` as unexpectedly skipped. |
| Architecture and project context | Yes | Preserve T-0063's exact-commit CI-reuse design and add the missing final-job override. |
| Data, security, and permissions | Yes | No permission changes; publication remains gated on successful build aggregation and cancellation state. |
| Slices and ownership | Yes | One workflow condition plus directly owned verification and durable records. |
| Verification and rollback | Yes | Static counterfactual check, actionlint, mandated project checks, and a one-line workflow rollback. |

Readiness verdict: Ready

## Verification Results

| Required? | Check Or Method | Observed Result | Status | Unblocking Condition If Not Run |
| --- | --- | --- | --- | --- |
| Yes | Counterfactual static contract check | Failed before the workflow fix because `Create Release` lacked the explicit publication condition; passed afterward. | Pass | Not applicable. |
| Yes | `bash -n scripts/check-release-workflow.sh` and `shellcheck scripts/check-release-workflow.sh` | Passed; ShellCheck 0.10.0 reported no findings after correcting one informational quoting issue. | Pass | Not applicable. |
| Yes | `actionlint .github/workflows/*.yml` | Passed for all workflows with `actionlint 1.7.12`. | Pass | Not applicable. |
| Yes | `cargo fmt --check` | Passed with no diff. | Pass | Not applicable. |
| Yes | `cargo clippy --all-targets -- -D warnings` | Passed with warnings denied across all targets. | Pass | Not applicable. |
| Yes | `cargo test` | Passed: 511 unit tests and every runnable integration test; 3 CLI and 32 Docker-dependent tests remained intentionally ignored. | Pass | Not applicable. |
| Yes | Focused diff and release-gate review | Confirmed exact-commit CI reuse and direct-tag CI behavior are unchanged, publication requires successful aggregate builds, permissions are unchanged, and no release/push action was performed. | Pass | Not applicable. |

- Criteria or methods amended after implementation began, with reason and impact: None.
- Counterfactual evidence for new regression or behavior tests: The new contract check failed against the diagnosed workflow with `workflow contract violation: .github/workflows/release.yml job release must contain: if: ${{ !cancelled() && needs.build.result == 'success' }}`, then passed after the one-line condition was added.
- Flaky result and disposition: None.

## Review Findings

| Severity | File Or Area | Finding | Required Fix | Status |
| --- | --- | --- | --- | --- |
| High | `.github/workflows/release.yml` | The intentionally skipped `ci` ancestor poisons the publication job's implicit `success()` even after all build matrix jobs pass. | Add an explicit status function and require the aggregated build result to be `success`. | Resolved |

## Consistency

| Canonical Source | Expected | Actual | Required Update |
| --- | --- | --- | --- |
| User request | Fix the skipped release publication step. | Final publication now has an explicit cancellation- and build-result-aware condition. | None. |
| Project brief or task brief | Automatic releases reuse exact-commit CI without weakening the publication gate. | Reuse is unchanged; publication proceeds only after successful builds. | None. |
| Decisions and standards | Release publication requires green CI and builds; no push or release without explicit authority. | Permissions are unchanged, the threat model is updated, and no external action was performed. | None. |
| Tests and docs | Workflow contracts should protect the final publication edge. | The contract script is run by CI's format job and documented in the command catalog. | None. |
| State and assumptions | T-0067 closes and T-0057 returns as the primary ready task. | Catalog and cursor reconciled. | None. |

## Batch And Residual Risk

- Large-diff split trigger hit: No
- If kept together, why: Not applicable.
- Risk not resolved by passing checks: Local static checks cannot execute GitHub's hosted scheduler; the next authorized release run supplies live confirmation.

## Completion

- Required checks all passed: Yes
- Status: Done
- Exact incomplete condition, if not Done: None.
- Next action: Stop; the next owner-authorized automatic release supplies live GitHub scheduler confirmation.
