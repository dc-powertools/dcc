# Project Onboarding

Run this procedure once when the framework enters a repository, and repeat only when a
major repository change makes the recorded context unreliable. The Root Orchestrator
owns onboarding and records the result in `.meta/README.md`.

## Procedure

1. **Initialize task discovery and the project cursor.** A valid cursor begins with
   `# Project State`. If `.meta/README.md` is absent, instantiate it from
   [templates/project-state.md](templates/project-state.md). If that path contains
   other documentation, never overwrite it: inventory the content, relocate it to an
   appropriate canonical home under `.meta/project/`, update repository-local inbound
   links, and then instantiate the cursor. If relocation could break an external link,
   published contract, or tool, record the collision and obtain the owner's destination
   decision first. Never copy state from the framework source repository or another
   project. Preserve an existing valid cursor as project evidence. A valid catalog
   begins with `# Task Catalog`. If `.meta/tasks/README.md` is absent, instantiate it
   from [templates/task-catalog.md](templates/task-catalog.md), capturing the onboarding
   request as the first task. If that path contains other documentation, never overwrite
   it: inventory and relocate it to an appropriate non-catalog home, then update
   repository-local inbound links. If relocation could break an external link, published
   contract, or tool, record the collision and obtain the owner's destination decision
   before instantiating the catalog. If an established project already has task records,
   assign stable IDs and link them without inventing missing history.
2. **Inventory the repository.** Read instruction files, manifests, lockfiles, CI and
   release configuration, contributor docs, source entry points, tests, recent commits,
   and current working-tree state. Classify the project as greenfield or established.
3. **Derive the command catalog.** Use manifests, task runners, and CI as candidates.
   Execute each safe local setup, run, and verification candidate in the current
   environment and record only observed successes in `.meta/project/standards.md`,
   created from [templates/standards.md](templates/standards.md). Do not exercise
   production, release, destructive migration, or external-action commands merely to
   catalog them; those need their established approval path and evidence. Include
   prerequisites and the verification date. Link to the catalog elsewhere.
4. **Ingest product and technical context.** Apply the source-trust rules in
   [knowledge-ingestion.md](knowledge-ingestion.md). Distill supplied documents and
   current repository evidence; do not copy source material wholesale.
5. **Resolve only material gaps.** First state the inferred default and its evidence.
   Ask at most three questions in one onboarding round, limited to answers that change
   the product outcome, safety boundary, architecture, or acceptance criteria. State
   what default will be used if the owner does not answer.
6. **Seed useful memory.** Create `.meta/project/brief.md`,
   `.meta/project/context.md`, assumptions, glossary, source map, standards, decisions,
   or other categorized project records only when the inventory produced real content.
7. **Prove cold-start readiness.** From the recorded files, confirm that a new agent can
   identify the product outcome, task catalog, primary and eligible tasks, dependency-
   blocked or parked work, next safe action, canonical commands, important constraints,
   and approvals without asking the owner to repeat repository-recoverable facts.

## Greenfield Variant

For a greenfield repository, there may be no commands or conventions to derive. Record
product and technology choices as decisions instead of presenting guesses as existing
facts. Add a command to `.meta/project/standards.md` only after its underlying tool
exists and the command has run successfully. Update project context as implementation
establishes real patterns.

## Established-Project Variant

Treat code, tests, and working CI as evidence of current behavior, not automatically as
desired behavior. Record compatibility constraints and conflicts between docs and code.
Do not replace established instruction files or command entry points; make
`.meta/project/standards.md` the catalog that points to the exact verified invocation.
