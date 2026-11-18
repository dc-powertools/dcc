# T-0032: Decide whether `postStartCommand` hook scripts should be baked into the image alongside the supervisor (T-0029) or remain host-generated on the `rt` bind mount as T-0028 provisionally concluded; the blocker is `${localEnv:VAR}`, which is only resolvable at run time from the invoking user's environment.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0032

## Outcome

Decide whether `postStartCommand` hook scripts should be baked into the image alongside the supervisor (T-0029) or remain host-generated on the `rt` bind mount as T-0028 provisionally concluded; the blocker is `${localEnv:VAR}`, which is only resolvable at run time from the invoking user's environment.

## Authority / Revision

User direction during T-0028 closure / r1

## Status

Done

## Depends On

T-0028

## Route / Risk

Design / Medium

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Decision 0004 already answers Q1–Q4 and T-0029 implemented it: startup hooks remain host-generated on the `rt` mount so `${localEnv:…}` stays runtime-correct; baking or a second in-container substitution engine remains rejected. The 2026-08-24 maintenance pass corrected the stale catalog status.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [whether to bake hooks brief](whether-to-bake-hooks-brief.md) — Scope, design or execution evidence.
- [decision 0004 embed supervisor in image](../0028-supervisor-delivery-model/decision-0004-embed-supervisor-in-image.md) — Historical decision rationale; canonical owner T-0028.

Additional original references: `.meta/archive/tasks/0032-whether-to-bake-hooks/whether-to-bake-hooks-brief.md`; `.meta/archive/tasks/0028-supervisor-delivery-model/decision-0004-embed-supervisor-in-image.md`.

## Provenance

Migrated from the T-0032 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
