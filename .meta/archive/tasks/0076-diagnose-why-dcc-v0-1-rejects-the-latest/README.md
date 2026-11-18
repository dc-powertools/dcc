# T-0076: Diagnose why dcc v0.1 rejects the latest dc-powertools Rust Feature when its `customizations.dcc.state` contains `${containerWorkspaceFolder}/target`.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0076

## Outcome

Diagnose why dcc v0.1 rejects the latest dc-powertools Rust Feature when its `customizations.dcc.state` contains `${containerWorkspaceFolder}/target`.

## Authority / Revision

User request 2026-08-25 / r1

## Status

Done

## Depends On

None

## Route / Risk

Discover / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; diagnosis complete and no product-code change was authorized.

## Result

Reproduced the exact failure with dcc 0.1.5 and a local Feature fixture before Docker invocation. Project state runs `apply_state_path_substitution` before validation, resolving `${containerWorkspaceFolder}` to `/workspace`; Feature state instead goes directly from raw metadata to `validate_state_entries_allowing_deferred_container_env`, which permits deferred `${containerEnv:...}` only and rejects the workspace token as unresolved. The Rust Feature's 0.1.0 migration introduced this state block on 2026-08-24, exposing an omission present since dcc first added Feature state support. The same defect affects any Feature state using fixed container variables, including the current Node Feature. The appropriate dcc correction is to apply container-side state-path substitution consistently before Feature validation (and cover raw Feature and imported metadata-label paths); a Feature-side literal `/workspace/target` is a narrow workaround. Focused project-state substitution passed, and no product code was changed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0076 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
