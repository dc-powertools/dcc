# T-0050: Add cross-layer tests proving runtime compatibility gates and CLI build/resource flags reach Docker with the correct arguments and conditions.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0050

## Outcome

Add cross-layer tests proving runtime compatibility gates and CLI build/resource flags reach Docker with the correct arguments and conditions.

## Authority / Revision

Parent T-0043; user follow-up 2026-08-24 / r1

## Status

Done

## Depends On

None

## Route / Risk

Initiative / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added `tests/docker_boundary.rs`, an argv-recording stateful fake Docker executable that drives the compiled CLI. Eight tests prove missing and major/minor-incompatible version labels refuse startup with the right rebuild instruction, patch drift reaches container creation, stop remains best-effort, `--no-cache` pulls image-profile and official upstream bases but not the generated intermediate, and explicit memory/CPU values reach `docker run` before image arguments. To satisfy the unspecified case, runtime resource flags are now optional and omitted from Docker unless supplied; docs and architecture no longer claim implicit 4g/2 limits. Focused fake-boundary and CLI tests, help output, format, clippy, full test (500 unit plus 8 fake-Docker and other integration suites), build, ordering review, and diff check passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [cli docker contract tests brief](cli-docker-contract-tests-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0050-cli-docker-contract-tests/cli-docker-contract-tests-brief.md`.

## Provenance

Migrated from the T-0050 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
