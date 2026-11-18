# T-0113: Repair startup/admission and wrapper-retirement races.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0113

## Outcome

Prevent startup/admission and premature wrapper-retirement races with a shared lifecycle lock and supervisor-owned PID/start-time registration cleanup.

## Authority / Revision

User scope-split, closure and archive request 2026-11-18 / r10. Supersedes r9's requirement to keep T-0113 open for deferred result-delivery work; that work is now owned by npspqj@r1.

## Status

Done

## Depends On

None

## Route / Risk

Initiative / High

## Approval Or Blocker

None for the completed startup/admission scope. Deferred post-exit Docker result delivery and separately deferred pre-command disconnection research transferred to [npspqj](../../../tasks/npspqj-command-result-delivery/README.md); neither was implemented or newly approved by this scope split.

## Next Safe Action

None; closed and archived. Resume deferred work only through npspqj and its recorded scope/authority.

## Result

Verified implementation delivered in local commit `29af4da`: shared lifecycle flock, atomic persistent wrapper PID/start-time records, supervisor-owned retirement, no ARRIVED file, and locked drain/promotion/stop. Version 0.4.0 fences incompatible 0.3 images. All 717 runnable default tests, 18 focused lifecycle tests (included in that total), 37 live Docker smokes, fmt, locked check/build and Clippy passed under r9; no new runtime verification is claimed here. Debian/Alpine root/non-root packaging and actual CLI/Docker-client loss were qualified. User-requested r10 scope separation transfers the unresolved post-exit hazard and pre-command disconnect research to npspqj, completing T-0113 without claiming those hazards fixed.

## Scope and acceptance

r10 closes the verified r9 startup/admission and wrapper-retirement repair, with unchanged implementation acceptance and observed checks. Deferred result delivery and pre-command disconnect research belong to npspqj@r1. The historical design/evidence retains earlier broader scope for provenance; it does not supersede this split or select a journal, CLI acknowledgment or disconnect-detection implementation.

## Supporting evidence

- [Execution r9](execution-r9.md) — Implementation, agent ownership, verification and review evidence.
- [quality teardown races](quality-teardown-races.md) — Verification and review evidence.
- [quality interactive disconnects](quality-interactive-disconnects.md) — Verification and review evidence.
- [fast command teardown race](fast-command-teardown-race.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0113-fast-command-teardown-race/fast-command-teardown-race.md`; `.meta/archive/tasks/0113-fast-command-teardown-race/quality-teardown-races.md`; `.meta/archive/tasks/0113-fast-command-teardown-race/quality-interactive-disconnects.md`.

## Provenance

Migrated from the T-0113 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.

## Closure r10

The user explicitly requested separating deferred work into a new task, then closing
and archiving T-0113. This narrows the completed outcome without changing its code,
verification criteria or observed results. All four supporting evidence files move with this
README; shared disconnect research retains T-0113 as its canonical historical owner.
Current architecture/security and task links now point to the new deferred owner or
the archived implementation evidence. Documentation-only checks cover link/anchor
resolution, unique IDs, task-index coverage, terminal placement, cursor bounds and
diff scope. Rust/Docker suites are not rerun for this documentation-only split.

Observed closure verification: all 163 local links/anchors in the 15 changed/new
Markdown files resolve; 128 authoritative task IDs (including legacy subtask suffixes)
are unique; both indexes cover their task namespaces exactly. T-0113 is Done/r10
in the archive, npspqj is Parked/r1 in current tasks, and the primary cursor is cleared
and within its hard budget. The two shared research files are preserved byte-for-byte.
Historical T-0120 migration-map paths/hashes remain unchanged as revision-specific
evidence. Diff whitespace and documentation-only scope checks passed.
