# T-0120: Migrate framework memory to task directories and a separate archive

## Identity and authority

- Initial revision: r1.
- Source: user discussion and task-recording request, 2026-11-18.
- Catalog: [task catalog](../../../tasks/README.md), which owns lifecycle state until migration.
- This request captures future migration work; it does not direct execution now.
- Preserve the existing primary task and all unrelated work.

## Target outcome

Organize task state and its supporting evidence as self-contained task directories.
Keep nonterminal work discoverable through a small current-task index and terminal
work through a separate archive index. Current project documentation owns applicable
product and process requirements; historical tasks explain how those requirements
evolved. New task identities must not require coordination through a shared counter.

## Rationale and observed baseline

The goal is useful organization and reliable retrieval, rather than fewer files.
Agents should recover current work without accidentally treating old execution state,
superseded analysis, or historical decisions as current instructions.

The initial read-only assessment found 140 Markdown files under `.meta`, including
58 task detail files plus a catalog, 31 quality records, 10 decisions, 5 threat models,
and one incident. The catalog held 125 unique task entries: 119 Done, one Superseded,
and five nonterminal. At about 101 KB across 165 lines, it demonstrates why line-count
budgets alone do not bound startup context. No archive directory existed.

The framework's shared next-number counter can allocate the same task ID on separate
branches. Its single-writer rule coordinates one orchestrator's workers, not independent
branches. Moving authoritative state out of the central catalog also reduces shared
edits, although indexes can still have ordinary merge conflicts.

Fifty-two catalog rows have no linked detail file. Every historical task must still
receive its own directory and README, preserving the information in its catalog row.
Legacy identities also include six T-0024-* subtasks; do not assume all IDs are numeric.
Refresh these inventory figures before implementation; they are assessment evidence,
not fixed migration totals.

## Agreed target design

```text
.meta/
  README.md                         # Bounded project entrypoint
  meta/                             # Reusable Markdown-only framework
  project/                          # Current project knowledge and requirements
  tasks/
    README.md                       # Current task discovery index
    <id>-<slug>/
      README.md                     # Authoritative task record
      <supporting-files>            # Optional, with useful descriptive names
  archive/
    tasks/
      README.md                     # Historical task discovery index
      <id>-<slug>/
        README.md
        <supporting-files>
```

### Identity

- New IDs are uniformly random strings of six lowercase ASCII letters (`[a-z]{6}`).
  Remove the sequential allocator and shared next-number field.
- Check IDs against both current and archived tasks before creating a task; regenerate
  on collision. Validate uniqueness at integration because independent branches may
  allocate the same ID. Never silently conflate tasks with matching IDs.
- Six letters provide 308,915,776 possibilities. Approximate birthday collision
  probabilities are 0.0025% at 125 tasks, 0.16% at 1,000, and 15% at 10,000 before
  collision checks. This is a practical choice, not a uniqueness guarantee.
- Preserve existing task identities, including subtask suffixes. Random IDs apply to
  new tasks after cutover; do not renumber historical tasks. Directory slugs aid humans
  and are not identity. Moving to the archive preserves task identity and revision.

### Task ownership and supporting material

- Each task README owns its identity/revision, authority, outcome, scope and acceptance,
  lifecycle status, dependencies, route/risk, approvals or blockers, next action, and
  results as applicable. Keep enough execution checkpoint information to resume safely.
- Supporting material is optional and proportional. Incident analysis, threat reviews,
  verification, designs, research, and execution notes can use descriptive filenames;
  no mandatory category names or empty report files.
- The README explains and links to each supporting file so an agent can select what
  to read. Required evidence and quality gates remain meaningful regardless of filename.
- Shared evidence has one canonical owner and explicit links from other tasks. Do not
  duplicate it or infer ownership solely from matching filename numbers.

### Indexes and retrieval

- The current index contains short task summaries, links, and optional search keywords.
  It does not own task status, dependencies, approvals, next action, or results.
- Keep the archive index separate; the current index has a brief link to it. Do not
  require agents finding current work to read historical entries.
- Current tasks include Pending, Ready, Active, Parked, Blocked, and Needs verification.
  Archive terminal tasks (Done, Cancelled, Superseded) after preserving useful current
  knowledge. Do not archive incomplete verification as completed work.
- Startup and current-state searches target current tasks and project guidance first.
  Consult archives for targeted historical evidence, prior failures, or conflicting
  decisions. A directory boundary alone does not prevent context contamination.
- Reopening an archived task preserves identity and updates its location and references.
- Keep global scheduling/pause state and the primary-task pointer at one explicit
  project-wide owner, preferably the existing project cursor. They are not task-index
  metadata. Preserve approval boundaries and revision binding during cutover.

### Decisions and current project knowledge

- Retire the separate mandatory decision-record system and top-level report categories
  as canonical storage requirements. Significant choices must be reflected in their
  owning current project or framework documentation.
- Task records hold rationale, alternatives, supporting analysis, and data. Git history
  provides provenance; current documents link explicitly to important task evidence.
- Preserve existing decision records as supporting task evidence, mapping their origin
  explicitly because decision numbering is independent of task numbering.
- Before archiving originating tasks, reconcile still-operative decisions and system-wide
  threat requirements into current documentation. Finishing a task does not invalidate
  its architectural or security consequences.
- Preserve superseded and rejected reasoning as history, with its historical meaning
  clear. Agents may investigate older conflicting decisions through the archive.

## Migration process

1. Re-inventory repository state, task entries, all report categories, entrypoints,
   templates, optional harness adapters, links, and external-to-`.meta` references.
   Preserve unrelated changes. Record an explicit old-path/task-to-new-path mapping,
   including catalog-only tasks, subtasks, shared reports, and independently numbered
   decisions. Do not start with a blind prefix-based move.
2. Finalize the ownership and retrieval contract above in the reusable framework.
   Update startup/bootstrap detection in `AGENTS.md` and framework onboarding so an
   index replacing `# Task Catalog` is recognized as onboarded. Align root-loop,
   knowledge-management, resumption, ingestion, automation, quality, improvement,
   agent guidance, and templates; remove obsolete catalog ownership and numbered-path
   instructions. Keep reusable policy separate from project facts and the core
   Markdown-only. No mandatory task database or runtime generator.
3. Create bundles for the currently open tasks first. Preserve exact authority,
   revisions, status, dependencies, approvals, blockers, pending checks, and selected
   approaches. Maintain a valid primary-task pointer and a coherent cutover boundary.
4. Create one directory and README for every historical task, including catalog-only
   tasks and subtasks. Transfer catalog facts without inventing evidence or upgrading
   completion claims. Move related artifacts according to the mapping, preserving
   historical content except for necessary navigation and provenance corrections.
5. Reconcile operative decisions and cross-cutting knowledge into current documentation;
   link important task analysis. Retire superseded storage rules and top-level category
   directories after their contents and references have been accounted for.
6. Move terminal task bundles into `.meta/archive/tasks/`. Build the separate archive
   index and small current index. Update relative Markdown links, literal paths,
   dependency/detail references, code comments, project cursor, and adapter references.
   Include this migration task itself in the mapping and archive it only upon completion.
7. Validate the migrated state and perform a cold-start recovery exercise. Record the
   migration mapping, checks, exceptions, and framework-change rationale as task
   evidence. Align existing framework audit/log requirements with the new ownership
   model rather than leaving contradictory decision-record requirements behind.

## Acceptance and verification

- [ ] Every pre-migration task has exactly one canonical README, including tasks with
  no former detail file; old identities, revisions, provenance, and results survive.
- [ ] New-task allocation uses six random lowercase letters and no shared counter;
  duplicate detection covers both current/archive namespaces and branch integration.
- [ ] Authoritative task state lives in task READMEs; both indexes contain discovery
  information only, with separate current and historical entry lists.
- [ ] Every supporting artifact is accounted for, discoverable from its owner, and
  linked where shared. No mandatory supporting filenames or empty report scaffolding.
- [ ] Current project/framework docs state applicable requirements; obsolete decision
  and report-category ownership rules are removed without losing historical evidence.
- [ ] All nonterminal tasks remain current; terminal bundles are archived with stable
  identities. Global stop state, primary pointer, approval gates, and dependencies survive.
- [ ] Relative links, anchors where affected, literal paths, unique IDs, dependency
  resolution/cycles, index coverage, and cursor pointers pass consistency checks.
- [ ] Startup instructions, onboarding, templates, resumption, and optional adapters
  agree on the new contract. Review byte/content budgets as well as line counts.
- [ ] A fresh agent can find current tasks, recover a selected task's next safe action
  and constraints, and find applicable project requirements without reading the archive.
  A separate targeted lookup can recover historical rationale and conflicting decisions.
- [ ] The final diff preserves unrelated work and changes no application behavior.
  Documentation/integrity checks are primary verification; run application tests only
  if implementation unexpectedly changes executable behavior.

## Scope and risk

Route: Initiative; risk: Medium, because losing authority or current requirements can
misdirect later agents even though the migration has little runtime coupling.
Scope includes repository memory, reusable policy/templates, entrypoints/adapters, and
necessary reference corrections. Product implementation, unrelated cleanup, publication,
and changes to existing authorization or quality boundaries are outside this task.

The principal risks are lost catalog-only history, archived-but-operative requirements,
duplicated state, mistaken report ownership, and broken links after moves. The explicit
mapping, current-knowledge reconciliation, integrity checks, and cold-start exercise
are the required controls. Migration is not complete merely when file moves succeed.
