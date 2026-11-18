# T-0048: Test and harden the OCI Feature acquisition trust boundary, including authentication, HTTP failures, digest verification, metadata validity, and archive safety.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0048

## Outcome

Test and harden the OCI Feature acquisition trust boundary, including authentication, HTTP failures, digest verification, metadata validity, and archive safety.

## Authority / Revision

Parent T-0043; user follow-up 2026-08-24 / r1

## Status

Done

## Depends On

None

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Added a deterministic local HTTP registry fixture covering reordered/optional Bearer challenges, URL-encoded token acquisition, token reuse, sanitized auth/manifest/blob failures, and a valid end-to-end acquisition; production token realms must use HTTPS. Centralized strict SHA-256 verification with a one-byte mutation control; malformed and schema-invalid supplied metadata now error while missing metadata remains documented install-only compatibility; archive extraction rejects traversal, absolute paths, links, and special entries. Focused trust-boundary tests, repeat fixture runs, format, clippy, full test (491 unit plus integration suites), build, diff, docs, and security review passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [feature acquisition trust tests brief](feature-acquisition-trust-tests-brief.md) — Scope, design or execution evidence.

Additional original references: `.meta/archive/tasks/0048-feature-acquisition-trust-tests/feature-acquisition-trust-tests-brief.md`.

## Provenance

Migrated from the T-0048 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
