# T-0077: Make Feature-contributed `customizations.dcc.state` apply the same supported container-side path substitution as project state, so `${containerWorkspaceFolder}` and other fixed container variables are resolved before validation.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0077

## Outcome

Make Feature-contributed `customizations.dcc.state` apply the same supported container-side path substitution as project state, so `${containerWorkspaceFolder}` and other fixed container variables are resolved before validation.

## Authority / Revision

User request 2026-08-25 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Centralized container-only state-entry substitution and applied it before validation for raw Feature metadata and nested or legacy imported `devcontainer.metadata` state. Fixed variables now normalize consistently with project state, deferred `${containerEnv:...}` remains image-aware, and host-local, reserved, relative, and overlapping paths retain their safety checks. Four counterfactual Feature tests failed on the old behavior; the corrected 61-test Feature module and full suite passed with 519 unit tests plus all runnable integration suites. Format, check, all-target Clippy, build, diff review, user/architecture documentation, and an independent zero-finding review passed. Live Docker/GHCR execution was not required for this pre-Docker metadata boundary.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0077 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
