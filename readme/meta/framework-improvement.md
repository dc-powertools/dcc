# Framework Improvement

Improve this framework when evidence shows avoidable mistakes, ambiguity, or drag. The
repository—not session memory—is the runtime for detecting recurrence and evaluating
whether a rule earned its context cost.

## Improvement Triggers

Consider a framework change when:

- the product owner repeats an instruction across tasks;
- `readme/learning/retrospectives.md` or its archives show the same correction or
  process failure more than once;
- a review, incident, near miss, escaped defect, failed release, or repeated failed run
  reveals a missing guardrail;
- work stalls because context, approval, or ownership has no clear home;
- a decision is rediscovered instead of referenced;
- a process is skipped because it is vague or disproportionate; or
- tooling changes how agents should build, research, or verify.

Do not create durable rules for one-off preferences unless the user explicitly asks.

## Evidence Ladder

| Level | Evidence | Default Disposition |
| --- | --- | --- |
| 4 | Repeated observed failure, accepted review finding, direct durable user instruction, or successful pilot | Adopt a scoped rule when its cost is justified |
| 3 | One concrete failure with a credible recurrence path | Adopt a very small control or Pilot |
| 2 | Strong primary-source practice or analogous project evidence | Pilot with an observable success signal |
| 1 | Plausible preference or speculative concern | Reject, defer, or gather evidence |

Evidence level informs judgment; it is not a numeric score. Name the decisive evidence.

## Change Lifecycle

1. **Find the signal.** Search the retrospective log, decisions, changelog, and current
   framework before adding guidance. Record new concrete learning in the retrospective.
2. **Screen the proposal.** Apply the hard rejects below. For a material change, state
   the problem, before/after behavior, affected canonical owners, context cost,
   verification, and possible sunset.
3. **Choose a disposition.** Use Adopt, Pilot, Revise, or Reject with decisive evidence
   and constraints. Direct user decisions and level-4 findings may be Adopted; reversible
   level-2 or level-3 defaults should usually be Pilots.
4. **Change the smallest owner.** Routine reversible edits can be applied directly.
   Create a decision record for significant process, safety, authority, or ownership
   choices. Keep templates aligned.
5. **Log every framework edit.** Append status, evidence, change, success signal, and
   review or sunset trigger to `readme/learning/framework-changelog.md`.
6. **Verify and close.** Run link, template, consistency, line-budget, and request review
   as applicable. All required checks must pass before Done.

This qualitative lifecycle replaces numeric self-scoring and dedicated Judge/Maintainer
roles. A focused Reviewer or independent agent can still reduce anchoring for a material
change when risk and available tooling justify it; role ceremony is not mandatory.

## Hard Rejects

Reject or revise a proposal when it:

- contradicts the user, `AGENTS.md`, accepted ownership, or higher-priority policy;
- adds non-Markdown runtime behavior or dependencies to the portable core; optional
  vendor-native declarative adapters are allowed only under the contract in
  [agent-definitions.md](agent-definitions.md#optional-harness-adapter-contract) and may
  not add executable code, dependencies, policy ownership, or broader authority;
- duplicates guidance with a clear canonical owner;
- creates vague duties another agent cannot verify;
- requires product-owner babysitting for routine agent responsibilities;
- bundles unrelated deferred work;
- lowers a safety, verification, or documentation gate without evidence and replacement
  controls; or
- has context or maintenance cost disproportionate to its evidence.

## Pilot Rules

Every Pilot names:

- scope and owner;
- the behavior expected to change;
- an observable success or failure signal;
- a review date, task-count trigger, or event trigger; and
- the rule or artifact to remove if the signal does not justify promotion.

The scheduled hygiene pass in
[knowledge-management.md](knowledge-management.md#maintenance-cadence) reviews due pilots
and sunset triggers. Promote, revise, or remove them; do not let Pilot become permanent
by neglect.

## Context-Cost And Calibration Checks

Before accepting a change:

- prefer one canonical home and links over repetition;
- stay within the artifact budgets in
  [knowledge-management.md](knowledge-management.md#artifact-budgets-and-overflow);
- compress or remove superseded guidance in the same change;
- check verbosity bias: more prose is not more evidence;
- check position and author bias: compare options against the same evidence;
- choose the lower-ceremony adequate control on a close call;
- require a before/after scenario for a major change, or state why none is practical; and
- name the evidence that decides the disposition.

## Rule Quality Bar

A framework rule is actionable, verifiable, scoped, concise, evidence-based,
non-conflicting, canonically owned, and paired with a trigger or cadence when it requires
maintenance.

## Durable Retrospective

At the end of substantial work, ask whether something slowed the work, was caught late,
had to be inferred, supplied confidence, or exposed a missing check. When the answer is
a concrete cross-session learning, search for an earlier occurrence and append the
three-field entry to `readme/learning/retrospectives.md`. A repeated signal invokes
the improvement lifecycle; an isolated observation can remain evidence without forcing
a new rule.

## Consistency Audit

Before closing framework edits, confirm:

- root entrypoints and state point to the right owners;
- new guidance has one home and removed copies leave working links;
- templates match process docs and remain within the twelve-template catalog;
- significant choices and every framework edit are recorded;
- optional harness adapters remain thin, removable, schema-valid, and subordinate to
  their canonical Markdown owners;
- required checks passed and completion status is accurate;
- budgets, pilot terms, and maintenance triggers are explicit; and
- the portable core remains Markdown-only and the framework reduces product-owner
  maintenance.
