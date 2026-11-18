# AI Coding Meta-Framework

This directory is the self-contained, reusable entrypoint for a portable, Markdown-only
framework core that gives coding agents an operating loop, durable project memory,
risk-scaled quality gates, and explicit autonomy boundaries without a runtime
dependency. Optional declarative harness adapters may expose selected roles without
becoming framework policy.

Every primary harness session and every delegated agent must read this file before task
work. This file explains what is reusable, what belongs to the host project, and which
process owner to load next.

## Startup Order

1. Read the applicable root `AGENTS.md` instructions.
2. Read this meta README in full.
3. Read `.meta/README.md` when it exists; it is the bounded current-project cursor.
4. Read `.meta/tasks/README.md` when it exists; it is the current task discovery index.
   Read the selected task README and current project requirements next. Consult archives
   only for targeted historical evidence, never as mandatory startup context.
5. Read only the process and project documents relevant to the assignment.

If `.meta/README.md` is missing or does not begin with `# Project State`, or
`.meta/tasks/README.md` is missing or does not begin with `# Current Tasks`, the add-on
is not fully onboarded. A primary session follows [onboarding.md](onboarding.md),
preserving colliding documentation, and instantiates the missing cursor or index. A
delegated agent does not initialize shared documentation unless the orchestrator
assigned that ownership.

## Directory Contract

`.meta/meta/` contains only reusable framework policy, references, and blank templates.
Project facts, decisions, commands, active work, reviews, learning, and archives never
become part of the reusable package.

The host project's agent-maintained documentation uses these mutable paths:

| Path | Purpose |
| --- | --- |
| `.meta/README.md` | Bounded project cursor, primary-task pointer, global scheduling and approvals |
| `.meta/project/` | Current project knowledge, applicable requirements, standards and command catalog |
| `.meta/tasks/README.md` | Current task discovery index; no lifecycle state |
| `.meta/tasks/<id>-<slug>/README.md` | Authoritative task state and links to optional supporting evidence |
| `.meta/archive/tasks/README.md` | Separate historical discovery index |
| `.meta/archive/tasks/<id>-<slug>/` | Terminal task bundles with stable identities and revisions |

The project cursor and current index are mandatory after onboarding. Create other
optional files and directories only when they will contain useful information.
Established host documentation may remain at its required conventional location; link
to its canonical owner instead of copying facts into framework-managed records.

## Optional Harness Integrations

The portable core is complete with this `.meta/meta/` tree and the merged root
`AGENTS.md` startup instruction. A root `CLAUDE.md` may import `AGENTS.md` so Claude Code
loads the same owner. Project files under `.codex/agents/` and `.claude/agents/` may
expose selected roles through native discovery.

These files are optional integration surfaces, not additional policy owners. They:

- point to [agent-definitions.md](agent-definitions.md) and other canonical process
  owners instead of copying their rules;
- keep agent adapters limited to vendor-required discovery metadata and least-privilege
  capability settings;
- do not add executable code, dependencies, model pins, MCP servers, hooks, permission
  bypasses, or integration ownership; and
- can be omitted or removed without changing the core framework workflow.

The current adapter pilot covers Reviewer, QA And Verification Agent, and Security And
Risk Agent. The source framework's current policy and originating task evidence own its promotion or sunset;
host projects may omit the pilot entirely.

## Principles

- Outcome first; context before code.
- One provisional workflow route, with risk as an independent safety overlay.
- Small reversible steps and observed verification results.
- Durable additive task intake with dependency- and safety-based selection, not FIFO.
- One canonical home for each fact, rule, decision, and command catalog.
- Repository-backed state and learning instead of assumed session memory.
- Agents maintain process memory; product owners make consequential product decisions.
- Repeated failures improve the system, with every local framework edit auditable.

## Process Map

- [root-loop.md](root-loop.md): additive intake, selection, and operating loop for every
  task.
- [workflow-routing.md](workflow-routing.md): single route table and escalation triggers.
- [onboarding.md](onboarding.md): cold-start inventory, command derivation, ingestion,
  state initialization, and cold-start proof.
- [knowledge-ingestion.md](knowledge-ingestion.md): source trust, synthesis, acceptance
  criteria, and conflict handling.
- [knowledge-management.md](knowledge-management.md): canonical artifacts, budgets,
  archives, and maintenance cadence.
- [automation-policy.md](automation-policy.md): standing authority, approvals, commands,
  and local commits.
- [resumption-protocol.md](resumption-protocol.md): task-targeted interruption and
  worker recovery.
- [agent-definitions.md](agent-definitions.md): optional roles, decomposition,
  integration, and shared-work safety.
- [development-standards.md](development-standards.md): default engineering standards.
- [quality-system.md](quality-system.md): risk gates, verification, review, security, and
  completion statuses.
- [framework-improvement.md](framework-improvement.md): evidence-based local framework
  edits, pilots, and sunset checks.
- [references.md](references.md): primary research basis.

## Template Catalog

- [project-state.md](templates/project-state.md) → `.meta/README.md`: bounded project
  cursor and documentation index.
- [project-brief.md](templates/project-brief.md) → `.meta/project/brief.md`: product
  outcomes and constraints.
- [project-context.md](templates/project-context.md) → `.meta/project/context.md`:
  concise technical conventions.
- [standards.md](templates/standards.md) → `.meta/project/standards.md`: project rules
  and verified command catalog.
- [assumptions.md](templates/assumptions.md) → `.meta/project/assumptions.md`:
  consequential uncertainty.
- [task-index.md](templates/task-index.md) → `.meta/tasks/README.md`: current discovery;
  use the same short-entry structure with `# Archived Tasks` for the separate archive index.
- [task-record.md](templates/task-record.md) → each task's `README.md`: authoritative state.
- [task-brief.md](templates/task-brief.md) and [task-notes.md](templates/task-notes.md):
  optional task-local scope detail and resumable checkpoints.
- [decision-record.md](templates/decision-record.md): optional task-local choice analysis;
  applicable requirements belong in current project/framework documentation.
- [quality-record.md](templates/quality-record.md),
  [threat-model-card.md](templates/threat-model-card.md), and
  [incident-note.md](templates/incident-note.md): optional task-local evidence structures.
  Required quality gates apply regardless of filenames or use of these templates.

Blank templates are schemas, not additional homes for project facts. The glossary and
source-map schemas live in [knowledge-management.md](knowledge-management.md); create
every project artifact only after it has useful content.

## Package And Bootstrap

A clean core package contains this `.meta/meta/` tree and a merged root AGENTS startup
instruction. For Claude Code, merge a root `CLAUDE.md` import of `AGENTS.md`. Optionally
merge the matching `.codex/agents/` or `.claude/agents/` when the destination uses those
harnesses. Never overwrite an established instruction or same-name agent.
The package excludes `.meta/README.md` and every mutable project-documentation sibling.
Packaging is the supported reset path; do not delete an existing project's documentation
to simulate a reset.

On first use:

1. The primary session reads root instructions and this file.
2. It runs [onboarding.md](onboarding.md) because the project cursor or current index is
   absent, or recognizes and safely resolves a non-framework document at either
   mandatory path.
3. It instantiates the project cursor and current index from
   [project-state.md](templates/project-state.md) and
   [task-index.md](templates/task-index.md).
4. It derives commands from manifests and CI, executes safe candidates, and records
   only observed successes in `.meta/project/standards.md`.
5. It creates project knowledge categories only when inventory produces real content.
6. It proves a new agent can recover the outcome, current index, primary and eligible tasks,
   next action, commands, constraints, and approvals from repository evidence.

For greenfield work, record product and technology choices as decisions rather than
pretending to derive them. For an established project, preserve existing instruction
and documentation owners and link them from the appropriate project records.
