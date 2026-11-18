# T-0113: Diagnose and fix fast-command teardown returning 137 after successful startup.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0113

## Outcome

Diagnose and fix fast-command teardown returning 137 after successful startup.

## Authority / Revision

User execution/orchestration request 2026-11-18 / r9

## Status

Ready

## Depends On

None

## Route / Risk

Initiative / High

## Approval Or Blocker

Verified startup/admission slice delivered under r9. Docker post-exit result delivery remains deliberately deferred; no result journal or transport design is selected. Pre-command disconnection detection also remains deferred.

## Next Safe Action

Select the design and scope for deferred Docker post-exit status/output delivery before claiming full T-0113 completion. Do not treat the verified startup/admission slice as authorization to implement the unselected journal or disconnect-detection work.

## Result

Implemented and independently reviewed the selected startup/admission repair: one lifecycle flock, atomic persistent wrapper PID/start-time records, supervisor-owned retirement, no ARRIVED file, and locked drain/promotion/stop. Version 0.4.0 fences incompatible 0.3 images. All 717 runnable default tests, 18 focused lifecycle tests (included in that total), 37 live Docker smokes, fmt, locked check/build and Clippy passed. Debian/Alpine root/non-root packaging and actual CLI/Docker-client loss qualified. The parent remains open for the deferred post-exit Docker result-delivery hazard; this is not a claim that every exit-137/output-loss case is fixed. See execution-r9.md for evidence and limits.

## Scope and acceptance

Execution r9 delivers the selected shared-lock/process-identity startup/admission slice. The original task remains open for the separately deferred post-exit Docker status/output hazard. The linked design and execution evidence define acceptance and verification; the verified slice does not select a journal, CLI acknowledgment or pre-command disconnect detection.

## Supporting evidence

- [Execution r9](execution-r9.md) — Implementation, agent ownership, verification and review evidence.
- [quality teardown races](quality-teardown-races.md) — Verification and review evidence.
- [quality interactive disconnects](quality-interactive-disconnects.md) — Verification and review evidence.
- [fast command teardown race](fast-command-teardown-race.md) — Scope, design or execution evidence.

Additional original references: `.meta/tasks/0113-fast-command-teardown-race/fast-command-teardown-race.md`; `.meta/tasks/0113-fast-command-teardown-race/quality-teardown-races.md`; `.meta/tasks/0113-fast-command-teardown-race/quality-interactive-disconnects.md`.

## Provenance

Migrated from the T-0113 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
