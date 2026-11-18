# T-0096: Design the approved container-managed relay and general configuration snapshot/drift behavior; pause for material complexity or unresolved ambiguity.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0096

## Outcome

Design the approved container-managed relay and general configuration snapshot/drift behavior; pause for material complexity or unresolved ambiguity.

## Authority / Revision

User request plus stop-before-build and explicit socat -t 2 selection 2026-09-28 / r4

## Status

Done

## Depends On

T-0095

## Route / Risk

Decide / High

## Approval Or Blocker

None

## Next Safe Action

Stop; design complete, implementation remains separate.

## Result

Designed packaged socat -t 2 under the existing supervisor, deterministic configurable proxy ports, immutable launch snapshots and cheap drift checks, simple stop-before-build guard, bounded recovery and safe creation fallback. Architecture, security, source/policy consistency, local-link and diff reviews passed. Implementation slices and platform qualification matrix recorded; no product implementation, reproduction, runtime tests, version bump or release.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality container relay design](quality-container-relay-design.md) — Verification and review evidence.
- [container relay design](container-relay-design.md) — Scope, design or execution evidence.
- [threat container relay design](threat-container-relay-design.md) — Security and trust-boundary analysis.

Additional original references: `.meta/archive/tasks/0096-container-relay-design/container-relay-design.md`.

## Provenance

Migrated from the T-0096 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
