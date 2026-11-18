# yspqig: Resolve migration review findings

## Identity And Authority

- ID: yspqig
- Revision: r1
- Source and date: User request to apply the three reviewed fixes, 2026-11-18.
- Parent or split task: Follow-up to T-0120; no change to its historical completion.
- Status: Done
- Dependencies: None
- Route / risk: Quick change / Low
- Approval or blocker: None
- Next safe action: Stop; all three accepted corrections are complete.
- Results: All three corrections applied; documentation integrity and consistency checks passed.

## Outcome, Scope And Acceptance

Resolve the accepted review findings: allow required checkpoint/readiness content in
READMEs or optional supporting files; remove T-0013's obsolete table escape; explicitly
scope documentation-only verification in current development guidance. Keep public
maintainer guidance consistent. Preserve all unrelated task gates and the primary cursor.

Required verification: documentation links, ownership/template consistency, affected
line/byte budgets, whitespace and scope review. No application behavior changes;
Rust checks are not required for this documentation-only correction.

## Framework Change Audit

Adopt, evidence level 4: accepted review finding and explicit user direction.
Before: routing/resumption require separate artifacts despite optional-file policy.
After: required content may live in the task README or optional linked files.
Owners: workflow-routing.md, resumption-protocol.md and root-loop.md. No new files or context load
are required by the policy. Success signal: a complete README satisfies both rules.
Review at the next hygiene pass or if checkpoint recovery exposes missing evidence;
retain the evidence gates and revise wording if needed.

## Verification

Observed: all local Markdown link targets resolve; affected development-guide anchors
resolve; task IDs are unique across both namespaces and both indexes cover their
bundles exactly. Changed framework docs, task record and cursor meet line/byte budgets.
Reviewed optional-note and quality-record wording against the templates and quality
owner. Diff whitespace and scope checks pass; only Markdown files change. Rust checks
were not run under the explicit documentation-only verification rule. All unrelated
current task records, gates and primary selection remain unchanged.
