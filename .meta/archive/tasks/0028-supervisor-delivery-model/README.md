# T-0028: Reconsider the `rt` bind mount and supervisor delivery model now that removing the fast path lifts the constraint that forced it; decide between the read-only bind mount, baking assets into the image, or a hybrid, and record the outcome.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0028

## Outcome

Reconsider the `rt` bind mount and supervisor delivery model now that removing the fast path lifts the constraint that forced it; decide between the read-only bind mount, baking assets into the image, or a hybrid, and record the outcome.

## Authority / Revision

User direction 2026-08-13 / r1

## Status

Done

## Depends On

T-0027

## Route / Risk

Design / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Decided: **bake the supervisor scripts into the image, keep `postStartCommand` hook scripts on the `rt` bind mount**, and add a semver compatibility gate. Q1: version skew handled by interpreting the `dcc.version` label with semver rules (patch = compatible, major/minor or missing label = refuse) instead of the current warning; patch releases may not change the host↔supervisor protocol. Q2: root-user tampering with baked scripts declared an explicit non-goal; the read-only mount property was found not load-bearing for the threat model. Q3 (decisive): hooks cannot be baked because `apply_substitutions` resolves `${localEnv:VAR}` in `lifecycle` from the invoking process's env, so hook text is only fully resolvable at run time — the codebase already encodes this rule by denying `localEnv` to `containerEnv`, which *is* baked. Also found that the baked build-prep `.sh` assets are never executed (the host `docker exec`s a freshly substituted argv), so they are vestigial. Q4/Q5/Q6 excluded from the evaluation per owner direction. Corrected stale rationale in T-0024 D1 and T-0025 D0/D2; updated the threat model scope and added an accepted-risk row. Implementation split to T-0029 and T-0030.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [decision 0004 embed supervisor in image](decision-0004-embed-supervisor-in-image.md) — Historical decision rationale.
- [supervisor delivery model brief](supervisor-delivery-model-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0028-supervisor-delivery-model/supervisor-delivery-model-brief.md`; `.meta/archive/tasks/0028-supervisor-delivery-model/decision-0004-embed-supervisor-in-image.md`.

## Provenance

Migrated from the T-0028 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
