# T-0058: Diagnose the build issue evidenced by the latest CI logs in `logs_88701649099.zip`.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0058

## Outcome

Diagnose the build issue evidenced by the latest CI logs in `logs_88701649099.zip`.

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

Stop; diagnosis complete and no product-code change was authorized.

## Result

CI checkout `24d35e5` passed format, Clippy, ordinary tests, and build; only `forwarded_port_reaches_container_loopback_service` failed because it ended with an empty response after its read deadline. The relay correctly half-closes the connector's stdin, but its `docker exec -i … nc 127.0.0.1 PORT` command omits OpenBSD netcat's `-N`, so stdin EOF is not propagated as a TCP write-half close. A Docker-free reproduction showed the exact connector delivered both directions but timed out, while adding `-N` completed immediately. All six forwarding unit tests passed, confirming their fake upstream does not cover the real netcat process contract. Live Docker reproduction was unavailable because this nested environment denies daemon mount/unshare operations.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0058 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
