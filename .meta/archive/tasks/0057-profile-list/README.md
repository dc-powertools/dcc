# T-0057: Fully design and implement `dcc profile list` so users can discover available profiles, including stable text and JSON output, documentation, and automated verification.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0057

## Outcome

Fully design and implement `dcc profile list` so users can discover available profiles, including stable text and JSON output, documentation, and automated verification.

## Authority / Revision

User request 2026-08-24 / r1

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

Added nested `dcc profile list` discovery of direct selectable `.devcontainer/*.json` files, deterministic sorting, default annotation, filename-safe one-record-per-line text, and stable JSON records with name/config/default. Listing is read-only, Docker-free, works from nested directories, and bypasses irrelevant selected-profile resolution. Unit and CLI matrices cover filtering, symlinks, non-UTF-8/control names, empty results, output contracts, debug, and help. Format, check, all-target Clippy, 516 unit tests plus all runnable integration suites, build, manual output/help, docs, security, and diff reviews passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality profile list quality](quality-profile-list-quality.md) — Verification and review evidence.
- [profile list brief](profile-list-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0057-profile-list/profile-list-brief.md`; `.meta/archive/tasks/0057-profile-list/quality-profile-list-quality.md`.

## Provenance

Migrated from the T-0057 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
