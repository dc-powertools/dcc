# T-0056 Quality Record

- Date: 2026-08-24
- Change: Restore a contextual error for absent `${containerEnv:VAR}` references
  without an explicit default while preserving intentional empty and non-empty values.
- Route: Initiative
- Risk: Medium
- Owner or reviewer: T-0056 isolated implementer

## Scope And Criteria

- User-visible outcome: Runtime configuration fails clearly when a referenced image
  environment key is absent unless the token supplies an explicit default.
- In scope: the central resolver; state, workspace, runtime arguments, command,
  mount, environment, and lifecycle consumers; Feature equivalents; documentation;
  and decision history.
- Non-goals: `${localEnv:…}` behavior, config-load deferral, and bypassing
  post-substitution validation.

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Missing `${containerEnv:VAR}` without a default errors with variable and consumer context. | Resolver unit test plus fake-Docker consumer matrices. | The resolver names `MISSING`; project and Feature matrices cover workspaceFolder, runArgs, command argv, mounts, remoteEnv, state, and lifecycle hooks. | Pass |
| Explicit defaults remain fallbacks for absent keys. | Unit matrix and final Docker argv boundary test. | Both non-empty and explicitly empty defaults resolve; `ABSENT=fallback` reached `docker run`. | Pass |
| Present-empty and present-nonempty values are separate from absence. | Unit matrix and final Docker argv boundary test. | Present empty ignores a fallback and reached Docker as `EMPTY=`; present non-empty values win with and without defaults. | Pass |
| Build/runtime and project/Feature consumers enforce one rule. | Fake-Docker runtime, Feature metadata, and `build --refresh-only` tests. | Every named path propagated the same resolver failure with field, key, index, state path, phase, or Feature context. | Pass |
| Structured validation still runs after successful substitution. | Existing and retained state tests. | Explicitly empty and defaulted values still reach reserved/absolute path validation. | Pass |
| Public/project guidance and decision history agree. | Documentation and stale-text review. | User guide and architecture state the strict contract; decision 0006 supersedes decision 0005; T-0051 keeps a subsequent-correction annotation. | Pass |

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | The product correction and T-0056 brief explicitly require missing-without-default failure and separate present-empty behavior. |
| Architecture and project context | Yes | One deferred resolver feeds every runtime/build-preparation consumer; callers already support fallible lifecycle mapping. |
| Data, security, and permissions | Yes | The fail-fast change prevents malformed runtime input and does not add privileges, persistence, or external integrations. |
| Verification and rollback | Yes | Docker-free final-argv tests observe cross-layer behavior; reverting the task commit restores the prior compatibility policy. |

Readiness verdict: **Ready**.

## Verification Results

| Required? | Check Or Method | Observed Result | Status | Unblocking Condition If Not Run |
| --- | --- | --- | --- | --- |
| Yes | Counterfactual old absent-to-empty resolver branch | The resolver regression failed with `Ok("ab")`; the runtime consumer matrix failed because `start` succeeded instead of rejecting the missing value. | Pass | N/A |
| Yes | Focused resolver, state, lifecycle, runtime consumer, Feature consumer, and build-preparation tests | All passed after restoring the strict branch. | Pass | N/A |
| Yes | `cargo fmt --check` | Passed. | Pass | N/A |
| Yes | `cargo check` | Passed. | Pass | N/A |
| Yes | `cargo clippy -- -D warnings` | Passed with warnings denied. | Pass | N/A |
| Yes | `cargo test` | Passed: 504 unit; 31 runnable CLI with 3 ignored; 9 config; 13 fake-Docker; 9 Feature CLI; 32 Docker smokes compiled and ignored. | Pass | N/A |
| Yes | `cargo build` | Passed. | Pass | N/A |
| Yes | `git diff --check` and scoped diff review | Passed; all resolver callers were enumerated and no unrelated implementation change was found. | Pass | N/A |

- Criteria or methods amended after implementation began, with reason and impact:
  Feature-derived consumers and build-preparation lifecycle hooks were added to the
  boundary matrix after the initial project-runtime matrix passed, matching the brief's
  explicit all-consumer requirement.
- Counterfactual evidence for new regression or behavior tests: temporarily restoring
  the old `None => ""` branch caused both the direct regression and public runtime
  boundary test to fail; reinstating the strict branch made them pass.
- Flaky result and disposition: none.

## Review Findings

| Severity | File Or Area | Finding | Required Fix | Status |
| --- | --- | --- | --- | --- |
| N/A | Resolver call graph | Every direct caller now propagates `Result`; build/runtime lifecycle helpers already add phase/source context. | None | Resolved |
| N/A | Historical records | Rewriting T-0051 would erase the accepted sequence. | Preserve it and add explicit superseding links/annotation. | Resolved |

## Consistency

| Canonical Source | Expected | Actual | Required Update |
| --- | --- | --- | --- |
| User request and T-0056 brief | Missing-without-default errors; documented intended behavior. | Implementation, tests, and public docs match. | None |
| Decisions | Supersede the upstream-compatible T-0051 policy without rewriting history. | Decision 0006 supersedes 0005; 0005 content remains historical. | None |
| Project context/source guidance | Point future work at the strict local policy. | Context and source map now identify decision 0006 and product authority. | None |
| Tests | Cover absence, defaults, present-empty/nonempty, all consumers, and downstream validation. | Unit and fake-Docker matrices cover each class. | None |

## Batch And Residual Risk

- Large-diff split trigger hit: No. Source, tests, decision, and documentation form one
  indivisible substitution-contract correction.
- Risk not resolved by passing checks: live Docker smoke tests were not run in this
  isolated environment. The deterministic fake-Docker boundary proves inspected image
  values and errors reach the documented pre-creation boundary; Docker still owns real
  image inspection behavior.

## Completion

- Required checks all passed: Yes
- Status: Done
- Exact incomplete condition, if not Done: None
- Next action: Integrate the isolated task commit and let the root orchestrator close
  T-0056 in the catalog/cursor.
