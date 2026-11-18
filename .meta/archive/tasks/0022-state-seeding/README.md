# T-0022: Seed declared state from the image so Feature- and Dockerfile-installed content is not silently masked by empty bind mounts, and survives a wiped `.dcc`.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0022

## Outcome

Seed declared state from the image so Feature- and Dockerfile-installed content is not silently masked by empty bind mounts, and survives a wiped `.dcc`.

## Authority / Revision

User request 2026-07-21 / r1

## Status

Done

## Depends On

T-0021

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added `src/seed.rs`: `dcc build` hydrates declared state from the image via a one-shot container (state mounts off, host state root at `/dcc-seed`) copying with `tar` inside the container before build-prep hooks; a `dcc.seed` image label carries the resolved manifest; a host-side `.dcc/<profile>.seed.json` ledger records `seed_digest`/`build_id`; re-seed policy (match→skip, differ→preserve+warn, `--reseed-state`→overwrite); runtime guard on start/run/exec/attach warns on `build_id` mismatch and hydrates unrecorded entries; `--dry-run` reports planned seeding. Added `--reseed-state` CLI flag, `docker::run_to_completion`, and 8 ignored Docker smoke tests. Updated README and architecture. Verified fmt, clippy, tests, build, help output, and dry-run JSON. Live Docker execution delegated to CI.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [decision 0001 state seeding from image](decision-0001-state-seeding-from-image.md) — Historical decision rationale.
- [state seeding brief](state-seeding-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0022-state-seeding/state-seeding-brief.md`; `.meta/archive/tasks/0022-state-seeding/decision-0001-state-seeding-from-image.md`.

## Provenance

Migrated from the T-0022 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
