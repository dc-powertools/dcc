# Resumption Protocol

Interrupted work resumes from durable repository state, not the product owner's memory.
This protocol applies after device or network loss, context compaction, tool failure,
agent restart, approval waits, and deliberate user interruption.

## Interruption Types

| Type | Examples | Required Response |
| --- | --- | --- |
| Unplanned pause | Sleep, network loss, compaction, session restart | Reconstruct catalog, task, repository, and worker state; continue from the next safe action |
| Approval wait | Release, destructive action, sensitive permission | Park the gated task; continue independent eligible work from a clean boundary |
| Task guidance | Changed constraint, goal, example, or priority for a task | Halt conflicting task work, record the amendment, and reframe only that task |
| Global user stop | Stop all, pause, wait, hold on | Stop nonessential work, checkpoint active tasks, suspend workers, and do not schedule work |
| Worker loss | Stale worker, missing result, crashed tool | Reconcile shared state; recover only work still needed and safe to own |
| Capacity wait | An applicable usage window is at least 95%, or required telemetry is unknown | Checkpoint and suspend workers; resume only after a fresh safe reading |

## Durable Recovery State

`.meta/README.md` is the first pointer and `.meta/tasks/README.md` is the canonical
task catalog. Read the catalog before task details. The catalog owns global scheduling
pause, stable identity and revision, authority provenance, outcome, lifecycle status,
dependencies, selected route/risk, task-specific approval or blocker, next safe action,
and detail/result links.

Create a brief when scope, acceptance, route, or risk needs detail. Create a task note
for long-running, risky, paused, or parallel execution. A task note contains the
execution checkpoint, work-item status, changed files, commits, observed checks, worker
roster, relevant decisions, failed approaches, parked-approval detail, and work still
needed. It links the catalog instead of copying catalog-owned fields.

## Resume Loop

1. Read `AGENTS.md`, the meta README, `.meta/README.md`, the task catalog, the primary
   task's brief or notes, and any newly delivered message.
2. Inspect `git status`, recent commits, relevant diffs, running tools, worktrees, and
   worker state. Do not repeat a risky side effect until its prior result is known.
3. Validate unique task IDs, authority provenance, current revisions, valid links and
   dependencies, no dependency cycle, and no more than one primary `Active` task.
   Quarantine unverifiable nonterminal tasks as `Blocked` before tool-using work.
4. Classify newly delivered messages using the rules below and persist every accepted
   independent task before continuing implementation.
5. Revalidate the active task's status, revision, accepted amendments, dependency
   results, approval source/action/boundary/revision, repository safety, plan, and next
   action.
6. Review dead ends and stale output. Retry only when new evidence addresses the
   observed failure; never integrate output invalidated for that task.
7. Re-run only checks needed to establish current state, then resume the active task or
   use the root loop to select an eligible one.
8. Refresh catalog, cursor, and task note after meaningful progress and before a long
   pause.

If multiple tasks incorrectly appear primary-`Active`, checkpoint them and reconcile
ownership before editing. Do not guess which dirty changes belong to which task.

## Delivered Message Semantics

Classify a new message by target and intent rather than treating recency as global
replacement:

- A status question creates no task. Answer it; compatible work may continue unless
  the user asked only for a report or pause.
- A new independent outcome creates a new task and does not preempt a safe active
  increment.
- Guidance or an approval naming a task ID, or unambiguously referring to one task,
  updates only that task. A material amendment increments its revision and records
  source, reason, and impact; re-run affected framing, approval, route, risk, worker,
  and verification gates.
- A pause, cancellation, replacement, or reprioritization with a clear task target is
  scoped to that task. Cancellation and supersession record a disposition; they do not
  silently undo commits or external effects.
- Unqualified `stop`, `pause`, `wait`, or `hold on` sets catalog scheduling to `Paused`,
  records its source or reason, and checkpoints active work. Only authoritative resume
  guidance returns scheduling to `Running`. An ambiguous cancellation, replacement, or conflict is checkpointed and
  clarified for the affected tasks rather than inferred globally.
- `cancel all`, `replace all`, or equivalent explicit global scope applies to every
  nonterminal task. Preserve terminal history, effects, and task-specific dispositions.

Approval or worker output is usable only when its recorded task ID and revision match
the catalog. Output becomes stale when its task revision, ownership, or stop state
invalidates it; unrelated task arrival does not stale it. A final response addresses
the current request and relevant catalog state; it never presents pending, parked,
blocked, or `Needs verification` work as completed.

## Worker And Worktree Recovery

[agent-definitions.md](agent-definitions.md#parallel-integration-and-recovery) owns WIP
limits, shared-file writers, isolated-context publication, integration checks, and
orphan recovery. Do not reassign work until repository and worker records show that it
is absent or unusable. Preserve unknown commits and changes until ownership is known.

Replacement work receives the task ID and revision, original goal, applicable
guidance, ownership boundary, prior evidence, repository state, what not to redo or
revert, verification, and handoff format. Do not recover cancelled, superseded, or
globally paused work.

## Resume Verification

Before closing resumed work, confirm that its latest accepted instruction is satisfied,
stale output was reviewed before use, checks cover both sides of the interruption, and
the catalog records every remaining verification, approval, dependency, or blocker.
