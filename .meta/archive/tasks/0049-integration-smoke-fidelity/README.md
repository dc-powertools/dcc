# T-0049: Replace false-confidence CLI and Docker smoke assertions with direct checks of success, identity, reuse, teardown, and container state; keep Docker-free dry-run tests in the default suite.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0049

## Outcome

Replace false-confidence CLI and Docker smoke assertions with direct checks of success, identity, reuse, teardown, and container state; keep Docker-free dry-run tests in the default suite.

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

Dry-run reports now expose the resolved container id. Path-profile integration tests require successful JSON output and compare the exact id across `id`, build, and stop. Seeding dry-run moved into the default suite and succeeds with an executable-free `PATH`; Docker fixture cleanup tolerates a missing Docker binary. Live smokes query label-selected immutable Docker instance IDs: immediate stop variants prove absence, one-shot drain captures and proves removal of the original instance without a fixed acceptance sleep, and durable reuse/`--keep` preserve identity plus a single-run hook counter. Retired host-bookkeeping assertions were removed. Format, clippy, full test (500 unit, 30 CLI plus 3 ignored, 9 config, 32 compiled Docker smokes, 6 feature), build, smoke listing, name/assertion review, and diff check passed; live smokes were not runnable because Docker is unavailable.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [integration smoke fidelity brief](integration-smoke-fidelity-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0049-integration-smoke-fidelity/integration-smoke-fidelity-brief.md`.

## Provenance

Migrated from the T-0049 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
