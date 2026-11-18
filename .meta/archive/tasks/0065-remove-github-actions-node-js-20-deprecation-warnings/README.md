# T-0065: Remove GitHub Actions Node.js 20 deprecation warnings from every workflow action.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0065

## Outcome

Remove GitHub Actions Node.js 20 deprecation warnings from every workflow action.

## Authority / Revision

User request 2026-08-24 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / Low

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Upgraded `actions/upload-artifact` from v5 to v6 and `actions/download-artifact` from v5 to v7. Upstream manifests confirm every external action now uses Node 24 or is composite. `actionlint 1.7.12`, the runtime audit, format, all-target Clippy, and the full test suite passed. One unrelated forwarding bind-release test failed once, then passed 20 focused runs and the complete rerun.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0065 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
