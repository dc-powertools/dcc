# T-0095: Examine relay placements and settle user-facing behavior for a relay managed inside the development container.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0095

## Outcome

Examine relay placements and settle user-facing behavior for a relay managed inside the development container.

## Authority / Revision

User request and relay/configuration-drift design decisions 2026-09-28 / r11

## Status

Done

## Depends On

None

## Route / Risk

Discover / Low

## Approval Or Blocker

None

## Next Safe Action

Stop; placement and behavior discussion complete, detailed implementation design pending.

## Result

Accepted existing-supervisor ownership, packaged proxy, configurable internal ports, readiness and bounded recovery, simple isolation tradeoff, general warn-and-defer configuration snapshots, explicit environment/address-family support, and minor-version migration through explicit rebuild/recreation. Documentation consistency and diff checks passed. No reproduction, runtime verification, product-code changes, version bump, or publication.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [decision 0008 relay in development container](decision-0008-relay-in-development-container.md) — Historical decision rationale.
- [decision 0009 warn and defer running config changes](decision-0009-warn-and-defer-running-config-changes.md) — Historical decision rationale.
- [relay placement](relay-placement.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0095-relay-placement/relay-placement.md`; `.meta/archive/tasks/0095-relay-placement/decision-0008-relay-in-development-container.md`; `.meta/archive/tasks/0095-relay-placement/decision-0009-warn-and-defer-running-config-changes.md`.

## Provenance

Migrated from the T-0095 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
