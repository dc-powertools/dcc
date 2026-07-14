# Automation Policy

Agents should remove maintenance work from the product owner wherever safe. The default is to proceed, verify, and record, not to wait for manual coordination.

The items under **Automatically Do** are standing approval for safe, reversible work
inside the user-defined scope. The **Stop Or Ask First** list is a required minimum, not
an exhaustive grant of authority: higher-priority instructions and materially harmful,
irreversible, or external actions still govern when a novel case is not listed.

## Automatically Do

Agents may do these without asking when they are relevant to the current task:

- Read project docs, source code, tests, and decision records.
- Search current external sources when facts may have changed.
- Create or update the task catalog, task briefs or notes, product briefs, assumptions,
  glossary entries, source maps, standards, and decision records.
- Add or update tests that verify touched behavior.
- Run build, lint, typecheck, test, format, and local app commands.
- Install local development utilities needed to inspect, test, or format the project when repository policy allows it.
- Refactor narrowly when required to implement the requested change safely.
- Patch this framework when a clear repeated gap or user preference should become durable.
- Read and maintain `readme/README.md`; append concrete learnings and framework edits to
  their canonical logs when triggered.
- Create a local, task-scoped commit after completing and verifying file changes in a Git repository, following [Local Commit Completion](#local-commit-completion).
- Continue after a clarification window using explicit assumptions.
- Resume interrupted work from repository state, task notes, plans, and agent rosters without asking the product owner to reconstruct context.
- Delegate bounded in-scope work under the standing request in root `AGENTS.md` when
  [the decomposition rules](agent-definitions.md#decomposition-rules) justify it and the
  [usage capacity guard](agent-definitions.md#usage-capacity-guard) permits it.
- Re-spawn stale or lost sub-agents only when their work is still needed and their ownership boundaries remain safe.
- Read authoritative five-hour and weekly usage, suspend or resume child workers, and
  set reset-aligned waits or polls under the
  [usage capacity guard](agent-definitions.md#usage-capacity-guard).

## Task Intake Persistence

The Root Orchestrator may persist newly delivered instructions to
`readme/tasks/README.md` without waiting for the active implementation task to finish.
Write the minimal row immediately as working-tree state and preserve it through
interruptions. Do not create a commit for an unfinished task merely to persist intake;
that requires separate explicit user direction or established repository authority.
At task completion, stage only that task's catalog hunks and preserve unrelated pending
rows. Intake does not mark a task complete or grant implementation authority.

## Local Commit Completion

A completed task that changes files in a Git repository must end with a local commit.
The agent does this without asking for separate approval after implementation and
verification are complete. Include that task's catalog/result update in the same
commit. Adjacent catalog rows or satisfied dependencies do not justify bundling
unrelated implementations.

Use this sequence:

1. Inspect repository status and the working diff, including knowledge or framework
   edits made late in the task. Run any checks those late edits require.
2. Select only files or hunks owned by the current task. Use explicit paths or another
   demonstrably task-scoped staging method. Never use convenience or blanket staging
   that could absorb unrelated user or concurrent-agent work.
3. Inspect the staged diff for scope, correctness, secrets, generated files, and other
   material that should not be committed.
4. Create a local commit with a clear message, using the repository's convention or the
   default in [development-standards.md](development-standards.md#git-and-change-management).
5. Inspect the resulting `HEAD` and repository status. Include the commit hash in the
   final response.

Do not commit when the user explicitly says not to, repository instructions prohibit
commits, the directory is not a Git repository, or a concrete technical or safety
blocker prevents a clean task-scoped commit. Pre-existing unrelated changes are not a
blocker when the task changes can be isolated, but they must be preserved and must not
be described as a clean worktree.

If an exception applies or any task-owned change remains uncommitted, the task is not
cleanly complete. The final response must name the uncommitted files and the exact
exception or blocker instead of claiming complete delivery.

This authority covers creating new local task commits only. It does not authorize
amending commits, rebasing, resetting, creating or switching branches, pushing,
releasing, deploying, or otherwise rewriting or publishing history.

## Stop Or Ask First

Agents must stop or ask before:

- Destructive data operations without a dry run, backup, or explicit instruction.
- Irreversible production actions.
- Publishing releases, sending external communications, charging money, or changing customer data.
- Adding high-risk production dependencies when no project policy covers dependency approval.
- Making legal, compliance, medical, financial, or employment decisions.
- Changing security boundaries or access policy without clear requirements or a decision record.
- Expanding CI/CD, deployment, credential, production, or agent-tool permissions without an accepted approval path.
- Violating the canonical source-trust and hostile-instruction rules in
  [knowledge-ingestion.md](knowledge-ingestion.md).
- Continuing a stale plan after the user says stop, pause, cancel, wait, hold on, or provides goal-changing guidance.
- Continuing when two explicit user instructions directly conflict.

For an unlisted action, proceed only when it is a normal, reversible implementation
step within the systems, data, and people the user placed in scope. Ask when it creates
material external state, irreversible impact, new authority, or a safety boundary the
user did not place in scope. This preserves autonomy without treating an omission from
the list as blanket permission.

Expanding standing authority or removing a stop boundary is a material policy change.
It requires explicit user direction or an accepted decision under the framework-change
process, and the edit cannot retroactively authorize the gated action that motivated it.

Task capture or selection never grants authority for an external, destructive,
privileged, or otherwise approval-gated action. Persist an approval's source, status,
action, boundary, and task ID/revision. A revision change requires revalidation before
the approval can authorize work.

## User Interrupt Handling

Follow [resumption-protocol.md](resumption-protocol.md#delivered-message-semantics), which
is the canonical owner for additive intake, task targeting, stop, redirect,
stale-worker, and final-response behavior.

## Automatic Knowledge Maintenance

At the end of each non-trivial task, the agent should decide whether to update:

- `readme/tasks/README.md` for task intake, lifecycle, dependencies, and results.
- `readme/README.md` for the primary-task pointer, recent outcome, and hygiene counter.
- `readme/project/brief.md` for stable product facts.
- `readme/project/assumptions.md` for unresolved uncertainty.
- `readme/project/source-map.md` for important sources and freshness.
- `readme/project/glossary.md` for domain vocabulary.
- `readme/decisions/` for meaningful choices.
- `readme/project/standards.md` for project-specific rules.
- `readme/tasks/` for briefs and resumable execution detail.
- `readme/learning/retrospectives.md` for concrete cross-session learning signals.
- This framework for process improvements.

If no durable knowledge changed, do not create noise.

## Automation Backlog

When an agent notices a repeatable manual step that cannot be automated immediately, it should record it in the relevant task note or `readme/project/automation-backlog.md`:

```md
## Automation Candidate
- Trigger:
- Manual step:
- Proposed automation:
- Expected benefit:
- Risk:
- Owner:
- Status:
```

Create `readme/project/automation-backlog.md` only after the first real candidate exists.

## Verification Automation

`readme/project/standards.md` is the sole canonical command catalog. Existing manifests, task
runners, CI files, or instruction docs remain executable sources, but the catalog links
their exact verified invocation and other docs link back to the catalog. Derive it by
following [onboarding.md](onboarding.md), not by copying aspirational commands.

- Install dependencies.
- Run unit tests.
- Run integration tests.
- Run end-to-end tests.
- Run type checks.
- Run lint and format checks.
- Build/package.
- Start local app.

Record a command only after executing it successfully in the relevant environment.
Include prerequisites, observed result, and verification date when environment or
version drift could matter.

If commands are missing or unreliable, agents should document the gap and improve the
command path when it is in scope.

## Human Attention Budget

Escalations should be concise and decision-oriented:

- State the decision needed.
- Give the default recommendation.
- Explain impact of each option.
- Ask only for information that changes the next action.

Do not ask the product owner to restate facts already available in the repository.

A required approval blocks the dependent action, not unrelated safe work within the
existing scope. Checkpoint the gated item, continue independent work when useful, and
batch compatible decision requests so the product owner can resolve them together.

Park each task-specific approval in the catalog and put necessary detail in its linked
task record. Reserve `readme/README.md` for global or cross-task approvals. Present a
decision in ten lines or fewer using:

```md
Decision: <what needs approval>
Proposal: <specific action>
Default recommendation: <yes/no and why>
If yes: <consequence>
If no: <consequence or fallback>
Needed by: <dependent action; unrelated work continues>
```
