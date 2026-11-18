# T-0047: Add meaningful coverage for port forwarding, including bind failures, bidirectional relay behavior, shutdown, and IPv4/IPv6 availability.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0047

## Outcome

Add meaningful coverage for port forwarding, including bind failures, bidirectional relay behavior, shutdown, and IPv4/IPv6 availability.

## Authority / Revision

Parent T-0043; user follow-up 2026-08-24 / r1

## Status

Done

## Depends On

None

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Relay startup now binds all requested IPv4 listeners before spawning, treats IPv6 loopback as an optional family, retains connection tasks for cancellation, kills connectors on drop, and joins listener shutdown. The copy contract half-closes each completed output while draining the other direction. Six bounded deterministic tests cover bidirectional bytes and response drain, later-port collision rollback, IPv6 degradation, connector failure, IPv4 startup, and active-task cancellation; five consecutive focused runs, format, and clippy passed. Added an ignored real `dcc exec`/Docker/`nc` round-trip smoke; it compiled but could not run because Docker is unavailable in this environment.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [port forwarding coverage brief](port-forwarding-coverage-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0047-port-forwarding-coverage/port-forwarding-coverage-brief.md`.

## Provenance

Migrated from the T-0047 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
