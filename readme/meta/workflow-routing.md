# Workflow Routing

Choose one route that best describes the work now. Routes are provisional: upgrade or
change the route when discovery, review, or testing exposes more ambiguity or impact.
Risk is not a second workflow taxonomy; it is an independent safety overlay used only
to select the gates in [quality-system.md](quality-system.md). Route each selected task
individually. Catalog status, dependency position, and backlog size are not routes.

## Route Table

| Route | What The Work Looks Like | Required Ceremony |
| --- | --- | --- |
| Quick change | Clear, contained, reversible change with known verification | Short task frame, focused change, relevant checks, diff review |
| Clarify | User outcome, scope, examples, or acceptance criteria are materially ambiguous | Defaults-first questions or documented assumptions, then a task brief and a new route |
| Discover | Goal is clear but current behavior, implementation surface, commands, or constraints are unknown | Repository or source findings, verified context, and a new route or bounded patch |
| Decide | Hard-to-reverse product, architecture, security, dependency, or feasibility choice | Options and consequences; decision record or disposable spike when the choice is significant |
| Initiative | New or cross-cutting product area requiring multiple artifacts or independently verifiable slices | Product/task brief, active task note, decisions as needed, readiness section in a quality record, sliced plan |
| Correct course | Guidance targeting the selected task, or new evidence, invalidates its scope, criteria, design, or verification | Task-scoped impact note, affected artifacts updated, revised plan and route |

Do not separately choose a work mode, scale path, and lifecycle phase. Specialist roles
and parallel workers are optional execution techniques within a route, not additional
classifications.

## Risk Overlay

After routing, identify the highest applicable Low, Medium, High, or Critical gate from
[quality-system.md](quality-system.md). A small patch can still be High or Critical when
it touches authentication, payments, destructive data operations, permissions, or
production. Risk may increase the required review and approval without changing the
route; ambiguity or scope discoveries may change both.

## Escalation Triggers

Re-route immediately when:

- an assumption changes promised behavior or safety boundaries;
- the change crosses an unexpected subsystem or ownership boundary;
- a required check is unavailable or exposes broader failure;
- implementation reveals a hard-to-reverse decision;
- user guidance targeting the selected task changes its goal, constraints, or
  acceptance criteria; or
- parallel work can no longer be integrated without overlapping ownership.

## Next-Action Router

At the end of substantial work, record one concrete next action in the selected task's
catalog row:

- implement the next ready slice;
- review a named diff or artifact;
- run a named verification command or method;
- clarify one material ambiguity;
- decide a named hard-to-reverse choice;
- correct a named upstream artifact;
- record durable knowledge; or
- stop because the goal is Done, Needs verification, Blocked, Cancelled, or parked for
  approval as defined by the owning process.

Name the command, artifact, decision, or condition that makes the action executable.
When a task closes or parks, recompute dependency and safety eligibility and select
another task through the root loop. Do not treat physical row order as priority.
