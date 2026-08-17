# Quality System

The quality system ensures the repository improves over time while preserving velocity. It combines automated checks, human-style review, risk gates, and consistency checks.

## Quality Gates By Risk

| Risk Level | Examples | Required Gate |
| --- | --- | --- |
| Low | Docs, comments, small styling, harmless config | Read-through and relevant lightweight check |
| Medium | Localized code behavior, non-critical UI, isolated refactor | Tests for touched behavior, lint/type/build as relevant, diff review |
| High | Auth, payments, data migration, security, public API, architecture, production ops | Decision review, expanded tests, security checklist, rollback or mitigation plan |
| Critical | Data deletion, legal/compliance impact, irreversible production action | Explicit user approval or established release process, dry run, backup/rollback, audit trail |

Use the highest applicable risk level.

## Risk-Based Approval Map

| Risk Level | Approval Path |
| --- | --- |
| Low | Implementer self-check, relevant lightweight verification, final summary |
| Medium | Implementer verification plus Reviewer or focused diff review |
| High | Reviewer plus Architect, QA, Security, or release owner as relevant |
| Critical | Explicit user approval or established release process before execution |

Escalate the path when the change is hard to reverse, touches multiple ownership areas, or skips a normally required check.

## Small-Batch And Large-Diff Triggers

Small-batch target: one user outcome, one process decision, or one refactor theme that a reviewer can understand without reconstructing the whole project.

Catalog adjacency, shared dependencies, or receipt in one message does not make
unrelated tasks one batch. Combine instructions only when they are deliberately
reframed before material work as one coherent outcome with compatible acceptance,
route, risk, verification, and commit boundaries.

Large-diff split triggers: split a change before review or commit when any trigger applies:

- The diff combines unrelated behavior, refactor, formatting, or documentation changes.
- The change touches more than two major subsystems or ownership areas.
- The review requires different specialist gates, such as security and UX.
- The verification matrix becomes too broad to run and explain clearly.
- A reviewer cannot summarize the intent and risk in a short paragraph.
- The diff is large enough that defects could hide in noise; prefer splitting around independently testable behavior.

If a split trigger is intentionally ignored, record the reason in the quality record or
final response.

## Standard Verification Matrix

For each task, decide which checks apply:

- Unit tests.
- Integration tests.
- End-to-end or browser tests.
- Type check.
- Lint.
- Format check.
- Build/package.
- Migration test.
- Accessibility check.
- Security check.
- Performance smoke test.
- Documentation link or command validation.
- Manual inspection for UI or workflow changes.

Declare which checks are required for the task before material implementation when
practical. Run the smallest required set that gives credible confidence. Mark unrelated
checks Not applicable; do not call a required check optional after seeing its result.

## Task Isolation

Each selected task has its own route, risk gate, acceptance criteria, verification
evidence, completion status, and commit boundary. The number or risk of pending catalog
tasks does not change the selected task's route or permit weaker gates.

A `Blocked` or `Needs verification` task blocks its dependents. Independent eligible
work may continue only when the repository is clean or its ownership and verification
are demonstrably isolated from failed, dirty, or overlapping work. Never run later
file-changing tasks through a broken shared baseline. If later work invalidates an
earlier task's acceptance, reopen it or create a corrective task and withhold any
aggregate completion claim.

`Done`, `Needs verification`, `Blocked`, `Cancelled`, and `Superseded` are task-scoped.
The presence of one `Done` task does not make pending catalog work complete, and the
presence of one blocked task does not stall unrelated safe work.

## Verification Outcomes

- **Pass:** the check ran and its observed result met the criterion.
- **Fail:** the check ran and did not meet the criterion; fix the issue or keep the task
  open or Blocked.
- **Not run:** name the concrete environment or access limitation and the condition that
  will unblock it. If the check is required, the task status is Needs verification.
- **Not applicable:** the check was not required for the scoped behavior or risk.

`Done` requires every required runnable check for that task to pass. A residual-risk statement records
what passing checks do not establish; it cannot replace a result. A genuinely impossible
required check produces an explicit **Needs verification** handoff, not Done. An
established approval path may change which risk is accepted, but must not relabel an
unexecuted required check as passing.

## Verification Integrity

For medium-, high-, or critical-risk behavior changes, and whenever verification could
be tailored to an already-known implementation, protect the integrity of the evidence:

- Declare acceptance criteria and the verification method before implementation when
  practical. If discovery changes them, record the reason and impact; use the
  Correct course route when the change affects scope or promised behavior.
- Treat unexplained weakening or removal of an acceptance criterion or planned check as
  a review finding. Legitimate amendments are allowed when their rationale is visible.
- Record observed results, not only commands that someone intended to run.
- For a new regression or behavior test, establish counterfactual confidence when safe
  and practical: show that it fails against the pre-change behavior, reproduce the
  failure before the fix, use a focused negative control or mutation, or explain why an
  equivalent method is more appropriate. Do not manipulate a shared worktree or risky
  environment merely to prove the counterfactual.
- A check that fails and then passes on unchanged code is evidence of a flake, not a
  clean pass. Investigate and record it. Quarantine only with a named owner or follow-up,
  bounded impact, and explicit residual risk; do not silently rerun until green.

## Quality Record

Use [templates/quality-record.md](templates/quality-record.md) when a change is high or
critical risk, major or cross-cutting, release-bound, unusually large, kept together
despite a split trigger, formally reviewed, or Needs verification. Its applicable
sections provide one home for readiness, acceptance, checks, review findings,
consistency, and residual risk. Low and medium changes may use the final response when
it records scope and observed required results.

## Implementation Readiness Gate

Use the readiness section of [templates/quality-record.md](templates/quality-record.md)
before implementation when work is major, cross-cutting, high-risk, ambiguous, or
depends on several upstream artifacts.

The gate checks whether product outcome, scope, acceptance criteria, architecture, project context, data/security concerns, slicing, verification, and rollback/readiness are sufficient. Verdicts:

- Ready: implementation can proceed.
- Ready with concerns: proceed only if concerns are recorded and bounded.
- Not ready: repair product, architecture, context, or slicing before coding.

Skip the gate for Quick change work unless review, testing, or user feedback shows the
plan is under-specified.

## Review Rubric

Review changes in this order:

1. Correctness: does it satisfy the request and acceptance criteria?
2. User impact: does the workflow make sense for affected users?
3. Safety: are data, permissions, secrets, and external actions protected?
4. Maintainability: is the design understandable and consistent?
5. Tests: would the tests fail for meaningful regressions?
6. Complexity: is this simpler than the problem requires?
7. Documentation: are changed behaviors and commands documented?
8. Consistency: does it match decisions, standards, and product language?

Prefer approving work that improves code health even if it is not perfect. Block issues that create real bugs, regressions, security risk, or misleading documentation.

Read the request, task brief, acceptance criteria, and applicable decisions before the
implementation diff when practical. This reduces anchoring on the builder's chosen
solution. Review verification amendments and counterfactual evidence as part of the
Tests item above.

## Structured Second Pass

For high-stakes product, architecture, readiness, or review artifacts, run one focused second pass instead of a vague "improve this" retry. Pick a lens that matches the risk:

- Pre-mortem: assume the plan failed and identify why.
- Inversion: ask how to guarantee failure, then avoid those causes.
- Adversarial review: require concrete findings or a justified zero-finding result.
- Stakeholder lens: re-check from user, operator, maintainer, buyer, or attacker perspective.

Treat second-pass findings as candidates, not truth. Filter false positives and keep only issues tied to the current scope.

## Security Checklist

Run for security-sensitive changes:

- External input validated on trusted side.
- Output encoded or escaped for target context.
- Authenticated routes require authentication by default.
- Authorization checks cover object-level and action-level access.
- Privileged logic is isolated and auditable.
- Secrets are not committed, logged, exposed to clients, or embedded in build output.
- Sensitive data is minimized, encrypted where required, and excluded from logs.
- Database calls use parameterized queries or safe ORM patterns.
- File upload/download paths validate type, size, path, permissions, and storage location.
- Errors do not expose stack traces, system details, or sensitive records.
- Dependencies and transitive risks are acceptable.
- AI tools cannot perform high-impact actions without appropriate guardrails.

## Threat Model Card Trigger

Use [templates/threat-model-card.md](templates/threat-model-card.md) before implementation or release when a change touches:

- Authentication, authorization, permissions, secrets, payments, personal data, or regulated data.
- External input, file upload/download, webhooks, plugins, browser automation, or AI tool calls.
- CI/CD, deployment, infrastructure, production operations, or cross-system trust boundaries.
- New dependencies, generated code paths, or agent instructions that could affect tool behavior.

Keep the card lightweight: identify what is being built, what can go wrong, what will be
done about it, and how the team knows the mitigations are enough. Use the quality record
when the risk is high or critical.

## Operational Readiness Mini-Gate

For production-impacting changes, verify before release:

- Rollback, disablement, or mitigation path is known.
- Logs, metrics, traces, or audit records can show whether the change works or fails.
- Operators can identify user impact and degraded states.
- Migrations, queues, retries, and background jobs have safe failure behavior.
- Configuration, secrets, and environment assumptions are documented.
- Alerts or manual checks cover the most important failure mode.

If a readiness item is required and cannot be checked, close as Needs verification and
record the reason in the quality record. Mark genuinely irrelevant items Not applicable.

## UI And UX Verification

For user-facing UI:

- Inspect desktop and mobile layouts.
- Check loading, empty, error, success, and permission states.
- Verify text fits containers and does not overlap.
- Verify keyboard and screen-reader basics for interactive controls.
- Confirm visual hierarchy fits the product domain.
- Test the primary workflow end to end.

## Release Readiness

Before release or merge, confirm:

- Acceptance criteria are met.
- Required tests and builds pass.
- Decision records are updated for significant choices.
- Documentation reflects current behavior.
- Rollback, migration, or mitigation path exists for high-risk changes.
- Monitoring, logging, or audit needs are covered.
- Known limitations are documented.

## Defect Handling Loop

When a defect is found:

1. Reproduce or characterize the failure.
2. Identify expected behavior from product context or user instruction.
3. Write or update a regression test when practical.
4. Fix the smallest responsible surface.
5. Run relevant checks.
6. Add a note to assumptions, standards, or decisions if the defect revealed missing knowledge.
7. Summarize impact and verification.

## Incident And Near-Miss Learning

Use [templates/incident-note.md](templates/incident-note.md) for production incidents, escaped defects with user impact, security near misses, failed releases, repeated failed agent runs, or checks that caught a serious issue late.

The note should be blameless and short. Capture impact, detection, timeline,
contributing factors, what worked, what failed, and concrete follow-up. At least one
follow-up should consider whether a test, standard, decision record, runbook, quality
record, or framework rule would prevent recurrence.

## Formal Review Record

Use the review section of [templates/quality-record.md](templates/quality-record.md)
when a change needs a durable formal review. Keep findings concrete and ordered by
severity.
