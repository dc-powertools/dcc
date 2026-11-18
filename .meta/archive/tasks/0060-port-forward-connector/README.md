# T-0060: Implement reliable port-forward half-close behavior through a baked variant-aware `dcc-connect` wrapper, compatible connector provisioning, Docker-free regression coverage, and aligned architecture documentation.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0060

## Outcome

Implement reliable port-forward half-close behavior through a baked variant-aware `dcc-connect` wrapper, compatible connector provisioning, Docker-free regression coverage, and aligned architecture documentation.

## Authority / Revision

User request and wrapper-scope follow-up 2026-08-24 / r2

## Status

Done

## Depends On

T-0059

## Route / Risk

Initiative / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Implemented a baked `dcc-connect` boundary selecting OpenBSD netcat with `-N`, identified Nmap Ncat without it, and generic `nc` only when standalone `-N` is advertised. Generated builds check compatibility before and after apt/apk/yum/dnf fallbacks and no longer accept arbitrary `nc`. The host relay now drops its child stdin pipe on request EOF while draining stdout. Eleven forwarding and 35 build-context tests cover selection, unsupported variants, exact argv, executable assets, ordering, and a real subprocess response-after-EOF path. Format, check, clippy, 510 unit plus all runnable integration tests, build, diff, architecture, and threat-model review passed. CI then ran `forwarded_port_reaches_container_loopback_service` successfully twice and passed all 32 Docker smokes in both runs.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality port forward connector quality](quality-port-forward-connector-quality.md) — Verification and review evidence.
- [port forward connector brief](port-forward-connector-brief.md) — Scope, design or execution evidence.
- [nc half close design](../0059-nc-half-close-design/nc-half-close-design.md) — Scope, design or execution evidence; canonical owner T-0059.

Additional original references: `.meta/archive/tasks/0060-port-forward-connector/port-forward-connector-brief.md`; `.meta/archive/tasks/0059-nc-half-close-design/nc-half-close-design.md`; `.meta/archive/tasks/0060-port-forward-connector/quality-port-forward-connector-quality.md`.

## Provenance

Migrated from the T-0060 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
