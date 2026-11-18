# T-0053: Add high-leverage property and behavior-matrix tests for config merging, shell quoting, Dockerfile ordering, seed digests, and Feature command editing.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0053

## Outcome

Add high-leverage property and behavior-matrix tests for config merging, shell quoting, Dockerfile ordering, seed digests, and Feature command editing.

## Authority / Revision

Parent T-0043; user follow-up 2026-08-24 / r1

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

Added fixed-seed bounded merge generators spanning scalar, build, map, list, Feature, lifecycle, and nested dcc families, with two-sided identity and full precedence/union/order laws; an inverted-precedence mutation failed and shrank after three cases. Shell quoting now round-trips an edge matrix and 96 bounded ASCII cases through `/bin/sh`. Build-context coverage requires the stage, version label, Feature and generated assets, plus Feature-before-netcat order. Seed digest tests prove content/path/symlink sensitivity, creation-order stability, and intentional permission-mode normalization, now documented. Feature CLI tests cover JSONC combined edits, duplicate coalescing, and byte-preserving no-op/missing removal summaries. Format, clippy, full test (502 unit, 9 Feature CLI, and other integration suites), build, and diff check passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [property and matrix tests brief](property-and-matrix-tests-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0053-property-and-matrix-tests/property-and-matrix-tests-brief.md`.

## Provenance

Migrated from the T-0053 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
