# T-0094: Diagnose attach failure when a running container has an already-bound forwarded host port, using source inspection without reproduction.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0094

## Outcome

Diagnose attach failure when a running container has an already-bound forwarded host port, using source inspection without reproduction.

## Authority / Revision

User request and no-reproduction constraint 2026-09-28 / r2

## Status

Done

## Depends On

None

## Route / Risk

Discover / Low

## Approval Or Blocker

None

## Next Safe Action

Stop; source-based diagnosis complete.

## Result

Container reuse still unconditionally acquires per-process forwarding listeners; a concurrent foreground session on the same ports therefore fails before attach hooks or the shell. Running-container state alone is insufficient: start creates no relays and completed sessions release them. Actual listener ownership on the affected host remains unknown. No reproduction or container launch was attempted; preparatory build cancelled. Source/documentation review and diff check passed; no product code changed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [attach port collision](attach-port-collision.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0094-attach-port-collision/attach-port-collision.md`.

## Provenance

Migrated from the T-0094 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
