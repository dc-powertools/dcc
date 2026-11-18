# T-0111: Reproduce cold one-shot startup status 137 and compare durable startup.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0111

## Outcome

Reproduce cold one-shot startup status 137 and compare durable startup.

## Authority / Revision

User request 2026-11-17 / r1

## Status

Done

## Depends On

None

## Route / Risk

Discover / Low

## Approval Or Blocker

None

## Next Safe Action

Stop; reproduction complete; product fix separate.

## Result

Reproduced exact startup 137 twice with Debian, sleep hook and forwarded port. Root-owned 0644 waiter FIFO blocks non-root supervisor notification; one-shot reaper kills readiness exec while container exits 0. Manual permission/notification experiment released durable startup. No product changes.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality startup reproduction](quality-startup-reproduction.md) — Verification and review evidence.

Additional original references: `.meta/archive/tasks/0111-reproduce-cold-one-shot-startup-status-137-and/quality-startup-reproduction.md`.

## Provenance

Migrated from the T-0111 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
