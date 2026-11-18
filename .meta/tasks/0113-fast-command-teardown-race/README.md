# T-0113: Diagnose and fix fast-command teardown returning 137 after successful startup.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0113

## Outcome

Diagnose and fix fast-command teardown returning 137 after successful startup.

## Authority / Revision

User shared-lock/process-identity acceptance and task-update request 2026-11-18 / r8

## Status

Ready

## Depends On

None

## Route / Risk

Initiative / Medium

## Approval Or Blocker

Keep open; product implementation pending; pre-command detection and post-exit Docker result work deferred

## Next Safe Action

Prepare selected shared-lock, supervisor-owned record retirement and PID/start-time approach for implementation; qualify startup regressions and image/protocol requirements.

## Result

Both original races and post-exit Docker result hazard documented. User selected startup/admission slice: exclusive lifecycle flock, supervisor retires persistent wrapper records using PID/start time, removes separate ARRIVED, and commits STOPPING under the lock. Retains container-owned cleanup without CLI acknowledgment. Result journal is not selected or a prerequisite; process-exit result work deferred. No product fix; task open.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality teardown races](quality-teardown-races.md) — Verification and review evidence.
- [quality interactive disconnects](quality-interactive-disconnects.md) — Verification and review evidence.
- [fast command teardown race](fast-command-teardown-race.md) — Scope, design or execution evidence.

Additional original references: `.meta/tasks/0113-fast-command-teardown-race/fast-command-teardown-race.md`; `.meta/tasks/0113-fast-command-teardown-race/quality-teardown-races.md`; `.meta/tasks/0113-fast-command-teardown-race/quality-interactive-disconnects.md`.

## Provenance

Migrated from the T-0113 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
