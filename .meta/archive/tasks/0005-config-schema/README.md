# T-0005: Implement devcontainer schema-compatible config parsing and merge behavior under `customizations.dcc`, including legacy `extends`/`scripts` warnings.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0005

## Outcome

Implement devcontainer schema-compatible config parsing and merge behavior under `customizations.dcc`, including legacy `extends`/`scripts` warnings.

## Authority / Revision

Parent T-0004 / r1

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

Added `customizations.dcc.extends`, `commands`, and parser-level `state` support; normalized legacy top-level `extends`/`scripts` with deprecation warnings; preserved existing command resolution semantics; verified fmt, clippy, tests, and build; completed read-only specialist review with no blocking findings.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [config schema brief](config-schema-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0005-config-schema/config-schema-brief.md`.

## Provenance

Migrated from the T-0005 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
