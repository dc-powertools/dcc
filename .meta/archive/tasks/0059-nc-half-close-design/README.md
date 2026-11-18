# T-0059: Design the `nc -N` port-forwarding correction, assess flag portability, and define Docker-free regression coverage.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0059

## Outcome

Design the `nc -N` port-forwarding correction, assess flag portability, and define Docker-free regression coverage.

## Authority / Revision

User request 2026-08-24 / r1

## Status

Done

## Depends On

T-0058

## Route / Risk

Decide / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; implementation-ready design complete.

## Result

Designed a narrow `nc -N` connector change, a pure argv contract test, a real loopback/subprocess fake-Docker test, retention of the in-memory relay test and Docker smoke, and architecture updates. Confirmed `-N` is OpenBSD-specific: BusyBox and traditional netcat omit it, while Nmap Ncat half-closes by default without that short flag. The accepted short-term design therefore records a real compatibility limitation for preinstalled `nc` and the existing yum/dnf `nmap-ncat` fallback.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [nc half close design](nc-half-close-design.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0059-nc-half-close-design/nc-half-close-design.md`.

## Provenance

Migrated from the T-0059 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
