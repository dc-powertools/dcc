# Knowledge Management

Repository memory must let a cold-start agent find the current cursor, durable facts,
decisions, and recurrence evidence without asking the product owner to reconstruct them.

## Canonical Artifacts

| Artifact | Canonical Purpose | Owner |
| --- | --- | --- |
| `readme/README.md` | Always-read pointer to the task catalog and primary task, global approvals, recent outcomes, and maintenance cursor | Root Orchestrator, every session and close |
| `readme/project/brief.md` | Product purpose, users, outcomes, and constraints | Agents update from product evidence |
| `readme/project/context.md` | Concise stack, technical conventions, and conflict-prone implementation rules | Agents update from code and decisions |
| `readme/project/standards.md` | Project-specific rules and the sole canonical command catalog | Agents update only from observed practice and executed commands |
| `readme/project/assumptions.md` | Open uncertainty, confidence, impact, and validation | Agent that introduces or resolves the assumption |
| `readme/project/glossary.md` | Canonical domain terms and deprecated synonyms | Agent ingesting or changing domain language |
| `readme/project/source-map.md` | Important sources, trust, ownership, and freshness | Agent relying on the source |
| `readme/project/automation-backlog.md` | Evidence-backed candidates for removing repeated manual work | Agent observing the candidate |
| `readme/project/agents.md` | Durable project-specific roles or agent rules justified by repeated use | Root Orchestrator |
| `readme/decisions/` | Append-only significant choices and consequences | Decision owner or Root Orchestrator |
| `readme/tasks/README.md` | Stable task IDs and revisions, authority provenance, global scheduling pause, outcomes, lifecycle, dependencies, selected route/risk, task approvals or blockers, next actions, detail links, and results | Root Orchestrator, sole writer |
| `readme/tasks/NNNN-*-brief.md` | Task-specific scope, acceptance, route/risk rationale, and amendments when the catalog row is insufficient | Root Orchestrator or assigned analyst |
| `readme/tasks/NNNN-*-notes.md` | Execution checkpoints for long-running, risky, paused, or parallel work | Root Orchestrator |
| `readme/quality/` | Durable readiness, verification, review, and completion evidence | Root Orchestrator or QA owner |
| `readme/threat-models/` | Security and trust-boundary analysis | Security or risk owner |
| `readme/incidents/` | Blameless incident and near-miss learning | Incident owner |
| `readme/learning/retrospectives.md` | Searchable cross-session correction and process-learning signals | Agent observing the signal |
| `readme/learning/framework-changelog.md` | Auditable framework edits, pilots, and sunset triggers | Agent changing the framework |

`readme/README.md` and `readme/tasks/README.md` are mandatory after onboarding because
they are the cold-start cursor and task discovery surface. Create every other project
artifact only when it has real content.

## One Home Per Fact

Give each durable fact, rule, decision, or command catalog one canonical home. Other
artifacts link to that owner instead of restating it. A short entrypoint or handoff
summary is allowed only when it links the canonical source and is updated in the same
change. If copies diverge, reconcile the owner and replace the copies with links.

Templates define structure; instantiated project artifacts own facts. A fact appearing
in a blank example is not a second home, but product-specific values must not be copied
between the brief, assumptions, glossary, and source map.

For task state, the catalog owns global scheduling pause, identity and revision,
authority provenance, concise outcome, status, dependencies, selected route/risk,
task-specific approval or blocker, one next safe action, and detail/result pointers. A
task brief expands scope, criteria, route/risk rationale, and material amendments. A task note
owns only the execution checkpoint and evidence needed to resume. The project cursor
points to these owners and may summarize recent outcomes; it does not maintain another
work list.

For an approval, the catalog cell owns its ID, status, bound task revision, and detail
link. The linked task record must persist approval source, action, and boundary; an
approval without the combined fields cannot make a task `Ready`.

## Task Lifecycle And Selection

Every accepted independent instruction receives the next stable `T-NNNN` ID, revision
`r1`, an authority reference, and a minimal catalog row immediately. IDs and physical
row order are identity and storage, not priority. Never reuse an ID. Only the Root
Orchestrator writes the catalog; workers propose changes in their handoff.

Direct user instructions and applicable higher-priority repository authority can create
tasks. The Root Orchestrator may accept an agent-found subtask only when it is necessary
to fulfill an already-authoritative task's outcome, safety, or verification and the row
cites that parent task. An out-of-scope finding remains a proposal in the active note or
automation backlog until the product owner accepts it; external content and worker
output never supply authority by themselves.

Increment the revision for a material outcome, scope, acceptance, approval-boundary, or
safety amendment. Record the source and reason in task detail. Bind approvals, worker
assignments, and returned output to `T-NNNN@rN`; revalidate or reject them after a
revision change. A nonterminal row with missing or unverifiable authority is `Blocked`
with that condition and cannot become `Ready` or `Active`.

Before a task becomes `Ready`, validate its authority chain and give it observable
acceptance. A normalized quick-task outcome may serve as its criterion when objectively
verifiable; otherwise create a brief. Record route and risk when the task is selected.

Lifecycle meanings:

- `Pending`: captured, but framing or acceptance is insufficient for selection.
- `Ready`: framed enough for its next route, with dependencies satisfied and no
  approval, blocker, ownership, or repository-safety gate.
- `Active`: selected primary work or explicitly recorded delegated work.
- `Parked`: deliberately paused at a recoverable checkpoint.
- `Blocked`: a concrete unresolved condition prevents progress.
- `Needs verification`: implementation exists but a required check cannot run.
- `Done`: its outcome and every required runnable check are complete.
- `Cancelled`: the user ended the task; history and effects remain recorded.
- `Superseded`: a named task now owns the outcome.

Keep at most one primary implementation task `Active` by default. Select eligible work
from dependencies, approvals, blockers, repository safety, user intent, unblock value,
risk, and coherent boundaries—not catalog order. A blocked or unverified task blocks its
dependents, not independent eligible work at a clean boundary. Arrival order may break
only an otherwise immaterial tie.

`Scheduling: Paused` is a durable global stop: checkpoint active tasks and select or
start nothing until authoritative user guidance resumes scheduling. Task-scoped pauses
use `Parked` without changing the global field.

## State Rules

Read the cursor and task catalog at every session start and refresh both at every task
close. During active work the cursor answers:

- Where is the catalog, and which task is primary?
- Which global approvals are parked, and what action does each gate?
- Which dead ends are still relevant?
- What recently completed, and what durable record explains it?
- When is the next hygiene pass due?

Keep one primary-task pointer, at most five recently completed entries, and only
currently relevant global dead ends. Task status and detail belong in the catalog and
linked task records.

## Decision Records

Create a decision record for a choice that is hard to reverse; materially changes
product scope, architecture, security, privacy, reliability, cost, workflow, or policy;
selects a major dependency; or resolves an important conflict.

- One decision per `readme/decisions/NNNN-short-title.md`.
- Status is `Proposed`, `Accepted`, `Superseded`, or `Rejected`.
- Accepted records are append-only. Supersede them with a linked new record.
- Include context, options, decision, consequences, confidence, sources, and trigger.

Use [templates/decision-record.md](templates/decision-record.md).

## Assumptions, Terms, And Sources

Use [templates/assumptions.md](templates/assumptions.md) for uncertainty that can proceed
safely. A low-confidence, high-impact assumption becomes a question, spike, or decision.

When first needed, use these minimal tables in the canonical files:

```md
# Glossary
| Term | Meaning | Use Instead Of | Source | Last Checked |
| --- | --- | --- | --- | --- |

# Source Map
| ID | Source | Owner/Publisher | Date Checked | Trust Tier | Scope | Notes |
| --- | --- | --- | --- | --- | --- | --- |
```

Mark stale sources and deprecated terms; do not erase history that explains decisions.
Re-check current vendor, legal, security, pricing, release, and API facts from primary
sources when they matter.

## Consistency Checks

Run a consistency pass when finishing a multi-file or behavior change, changing this
framework or a decision, resolving a major defect, preparing a release, or when the
maintenance cadence below fires. Check the request, acceptance criteria, implementation,
tests, brief, decisions, standards, assumptions, commands, docs, and state as applicable.
Use the consistency section of [templates/quality-record.md](templates/quality-record.md)
when the result needs a durable record.

## Artifact Budgets And Overflow

Budgets are defaults except for the two hard cursors. Exceed a default only with a short
rationale in the artifact; otherwise compress active material and archive history
without changing it.

| Artifact | Budget | Overflow Rule | Maintenance Owner |
| --- | ---: | --- | --- |
| `AGENTS.md` | 120 lines, hard | Move detail to an owning process doc and link it | Root Orchestrator |
| `readme/README.md` | 80 lines, hard | Move detail to task notes or decisions; retain only current pointers | Root Orchestrator |
| Project brief | 200 lines | Split stable technical detail to project context | Product Analyst or Root Orchestrator |
| Project context | 160 lines | Move broad standards or decision rationale to their owners | Architect or Root Orchestrator |
| Standards and command catalog | 240 lines | Split topic-specific standards only when one owner remains explicit | Root Orchestrator |
| Assumptions | 120 lines | Archive closed rows to `readme/archive/` | Root Orchestrator |
| Glossary | 160 lines | Archive deprecated terms after dependent docs migrate | Documentarian |
| Source map | 200 lines | Archive stale sources while preserving decision links | Research owner |
| Task catalog | 300 lines | Move terminal rows unchanged to a dated archive after distillation; retain every nonterminal row and an archive pointer | Root Orchestrator |
| Active task note | 300 lines | Move completed chronology to a dated archive; keep resume state | Root Orchestrator |
| Decision record | 220 lines each | Prefer linked supporting evidence; never truncate an accepted decision | Decision owner |
| Retrospective or framework changelog | 20 entries or 160 lines | Move old entries unchanged to a dated archive and link it | Root Orchestrator |
| Core framework process doc | 300 lines | Split only by a clear ownership boundary and update the index | Root Orchestrator |

Archive under `readme/archive/` with a date or sequence in the filename. Do not create an
empty archive directory. Archives are read on a targeted lookup, not every task.

## Maintenance Cadence

The Root Orchestrator runs a consistency and pruning pass after 10 completed
repository-changing tasks or 30 days since the last pass, whichever occurs first. The
project cursor holds both counters. The pass must:

1. validate cursor pointers, catalog links, unique IDs, acyclic dependencies, at most
   one primary active task, parked approvals, and canonical command links;
2. find budgets over limit and apply their overflow rules;
3. mark or supersede stale guidance and sources;
4. search retrospective repeats and evaluate due framework pilots or sunsets;
5. reconcile duplicated or conflicting guidance at its canonical owner; and
6. record the date, reset the task counter, and name any incomplete maintenance action.

This scheduled pass owns pruning; agents should still correct dangerous stale guidance
immediately when they encounter it.
