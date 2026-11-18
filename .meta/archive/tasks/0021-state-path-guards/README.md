# T-0021: Guard `customizations.dcc.state` against critical container paths, including the currently unguarded `/cache` self-nesting bug and `dcc`'s own `/usr/local/share/dcc` assets.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0021

## Outcome

Guard `customizations.dcc.state` against critical container paths, including the currently unguarded `/cache` self-nesting bug and `dcc`'s own `/usr/local/share/dcc` assets.

## Authority / Revision

User request 2026-07-21 / r1

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

Added two-tier reserved-path guards to `normalize_state_path` (subtree + exact), applied at config load and post-`${containerEnv:VAR}` resolution. Tier 1 blocks `/proc`, `/sys`, `/dev`, `/tmp`, `/run`, `/var/run`, `/var/lock`, `/boot`, `/bin`, `/sbin`, `/lib*`, `/usr/bin`, `/usr/sbin`, `/usr/lib*`, `/etc`, `/workspace/.dcc`, `/cache`, `/usr/local/share/dcc` (subtree). Tier 2 blocks `/usr`, `/var`, `/home`, `/root`, `/opt`, `/workspace`, `/srv`, `/mnt`, `/media` (exact only; children accepted). Errors name the path, matched reserved path, and an alternative. Updated README and architecture reserved-path docs. Verified fmt, clippy, tests (439 unit), and build.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [state path guards brief](state-path-guards-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0021-state-path-guards/state-path-guards-brief.md`.

## Provenance

Migrated from the T-0021 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
