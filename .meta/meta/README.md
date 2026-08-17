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
4. Read `.meta/tasks/README.md` when it exists; it is the canonical task catalog.
5. Read only the process and project documents relevant to the assignment.

If `.meta/README.md` is missing or does not begin with `# Project State`, or
`.meta/tasks/README.md` is missing or does not begin with `# Task Catalog`, the add-on
is not fully onboarded. A primary session follows [onboarding.md](onboarding.md),
preserving colliding documentation, and instantiates the missing cursor or catalog. A
delegated agent does not initialize shared documentation unless the orchestrator
assigned that ownership.

## Directory Contract

`.meta/meta/` contains only reusable framework policy, references, and blank templates.
Project facts, decisions, commands, active work, reviews, learning, and archives never
become part of the reusable package.

The host project's agent-maintained documentation uses these mutable paths:

| Path | Purpose |
| --- | --- |
| `.meta/README.md` | Always-read catalog and primary-task pointer, global approvals, recent outcomes, and documentation index |
| `.meta/project/` | Stable project brief, context, standards, assumptions, glossary, source map, automation backlog, and project-specific agent guidance |
| `.meta/decisions/` | Append-only significant project or local-framework decisions |
| `.meta/tasks/` | Canonical `README.md` task catalog plus proportional briefs and resumable notes |
| `.meta/quality/` | Durable readiness, verification, and review records |
| `.meta/threat-models/` | Lightweight security and trust-boundary analyses |
| `.meta/incidents/` | Incident and near-miss records |
| `.meta/learning/` | Retrospectives and the local framework changelog |
| `.meta/archive/` | Overflow moved from active artifacts without rewriting history |

The project cursor and task catalog are mandatory after onboarding. Create other
optional files and directories only when they will contain useful information.
Established host documentation may remain at its required conventional location; link
to its canonical owner instead of copying facts into framework-managed records.

## Optional Harness Integrations

The portable core is complete with this `.meta/meta/` tree and the merged root
`AGENTS.md` startup instruction. A root `CLAUDE.md` may import `AGENTS.md` so Claude Code
loads the same owner. Project files under `.codex/agents/` and `.claude/agents/` may
expose selected roles through native discovery. A repo skill under
`.agents/skills/codex-quota-monitor/` may expose the Codex-specific telemetry procedure
required by the portable usage capacity guard.

These files are optional integration surfaces, not additional policy owners. They:

- point to [agent-definitions.md](agent-definitions.md) and other canonical process
  owners instead of copying their rules;
- keep agent adapters limited to vendor-required discovery metadata and least-privilege
  capability settings, and the skill limited to its telemetry procedure and UI metadata;
- do not add executable code, dependencies, model pins, MCP servers, hooks, permission
  bypasses, or integration ownership; and
- can be omitted or removed without changing the core framework workflow.

The quota-monitor skill contains only its required Markdown instructions and UI metadata.
It uses the already-installed Codex App Server and links
[agent-definitions.md](agent-definitions.md#usage-capacity-guard) as policy owner; it is
not part of the three-role adapter pilot.

The current adapter pilot covers Reviewer, QA And Verification Agent, and Security And
Risk Agent. The source framework's decision and changelog own its promotion or sunset;
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
- [agent-definitions.md](agent-definitions.md): optional roles, decomposition, usage
  capacity, integration, and shared-work safety.
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
- [decision-record.md](templates/decision-record.md) → `.meta/decisions/`: significant
  choices.
- [task-catalog.md](templates/task-catalog.md) → `.meta/tasks/README.md`: durable task
  identity, lifecycle, dependencies, discovery, and results.
- [task-brief.md](templates/task-brief.md) and
  [task-notes.md](templates/task-notes.md) → `.meta/tasks/`: scoped outcomes and
  resumable work.
- [quality-record.md](templates/quality-record.md) → `.meta/quality/`: readiness,
  verification, review, consistency, and completion evidence.
- [threat-model-card.md](templates/threat-model-card.md) → `.meta/threat-models/`:
  lightweight agent-aware threats.
- [incident-note.md](templates/incident-note.md) → `.meta/incidents/`: blameless
  incident and near-miss learning.

Blank templates are schemas, not additional homes for project facts. The glossary and
source-map schemas live in [knowledge-management.md](knowledge-management.md); create
every project artifact only after it has useful content.

## Package And Bootstrap

A clean core package contains this `.meta/meta/` tree and a merged root AGENTS startup
instruction. For Claude Code, merge a root `CLAUDE.md` import of `AGENTS.md`. Optionally
merge the matching `.codex/agents/`, `.claude/agents/`, or Codex quota-monitor skill
when the destination uses those harnesses. Never overwrite an established instruction,
same-name agent, or same-name skill.
The package excludes `.meta/README.md` and every mutable project-documentation sibling.
Packaging is the supported reset path; do not delete an existing project's documentation
to simulate a reset.

On first use:

1. The primary session reads root instructions and this file.
2. It runs [onboarding.md](onboarding.md) because the project cursor or task catalog is
   absent, or recognizes and safely resolves a non-framework document at either
   mandatory path.
3. It instantiates the project cursor and task catalog from
   [project-state.md](templates/project-state.md) and
   [task-catalog.md](templates/task-catalog.md).
4. It derives commands from manifests and CI, executes safe candidates, and records
   only observed successes in `.meta/project/standards.md`.
5. It creates project knowledge categories only when inventory produces real content.
6. It proves a new agent can recover the outcome, catalog, primary and eligible tasks,
   next action, commands, constraints, and approvals from repository evidence.

For greenfield work, record product and technology choices as decisions rather than
pretending to derive them. For an established project, preserve existing instruction
and documentation owners and link them from the appropriate project records.
