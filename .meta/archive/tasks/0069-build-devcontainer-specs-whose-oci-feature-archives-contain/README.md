# T-0069: Build devcontainer specs whose OCI Feature archives contain a safe archive-root directory entry, including `ghcr.io/dc-powertools/features/sudo`, without weakening archive traversal or entry-type protections.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0069

## Outcome

Build devcontainer specs whose OCI Feature archives contain a safe archive-root directory entry, including `ghcr.io/dc-powertools/features/sudo`, without weakening archive traversal or entry-type protections.

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

OCI extraction now ignores normalized empty paths only for directory entries such as `./`; empty non-directory entries, traversal, absolute paths, links, and special entries remain rejected. The pre-fix regression reproduced the reported error, direct extraction and local authenticated-registry tests cover the root marker, and a temporary production-client check successfully downloaded and extracted `ghcr.io/dc-powertools/features/sudo:latest`. Format, check, all-target Clippy, 513 unit tests plus all runnable integration tests, build, and diff review passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

No separate supporting artifact was linked in the original catalog.

## Provenance

Migrated from the T-0069 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
