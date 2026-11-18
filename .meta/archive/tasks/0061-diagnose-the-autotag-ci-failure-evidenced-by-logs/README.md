# T-0061: Diagnose the autotag CI failure evidenced by `logs_88712697210.zip`.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0061

## Outcome

Diagnose the autotag CI failure evidenced by `logs_88712697210.zip`.

## Authority / Revision

User request 2026-08-24 / r1

## Status

Done

## Depends On

None

## Route / Risk

Discover / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; diagnosis complete and no product-code or workflow change was authorized.

## Result

The version bump pushed commit `07749f2` and autotag successfully created `v0.1.1`; the run failed afterward in release-gating CI. The sole failing check was `forward::tests::connector_prefers_openbsd_then_nmap_then_compatible_generic_nc` at checkout `0de0c24`, where spawning a just-written temporary `dcc-connect` script returned Linux `ETXTBSY` (`ExecutableFileBusy`). The same test and commit had passed minutes earlier, while format, Clippy, build, and both 32-test Docker-smoke runs passed. The fixture writes executable scripts while the parallel Rust suite spawns other processes, allowing a concurrent fork to transiently inherit a writable descriptor before `CLOEXEC`; this is a known `Command::spawn` race, not a connector behavior failure. Separately, release-gating CI checked stale commit `0de0c24` because its nested reusable CI checkout does not receive the tag ref, although the bump changed only version metadata.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0061 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
