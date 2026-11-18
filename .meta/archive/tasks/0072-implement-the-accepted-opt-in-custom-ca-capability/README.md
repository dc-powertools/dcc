# T-0072: Implement the accepted opt-in custom-CA capability in the production OCI Feature client with public defaults preserved, no insecure fallback, deterministic success/failure tests, and user/architecture documentation.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0072

## Outcome

Implement the accepted opt-in custom-CA capability in the production OCI Feature client with public defaults preserved, no insecure fallback, deterministic success/failure tests, and user/architecture documentation.

## Authority / Revision

Parent T-0070 / r1

## Status

Done

## Depends On

T-0071

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

Implemented canonical exact-authority `registryCAs`, declaration-relative inheritance, strict bounded PEM validation, additive authority-scoped rustls clients, safe manual redirects and token realms, sanitized diagnostics, Feature trust rejection, Docker-free TLS counterfactuals, dependencies, and documentation. Format, locked check, all-target Clippy, 555 unit plus 67 runnable integration tests, locked build, diff, dependency, security, and independent reviews passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality custom ca implementation quality](quality-custom-ca-implementation-quality.md) — Verification and review evidence.
- [custom ca registry initiative](../0070-custom-ca-registry-initiative/custom-ca-registry-initiative.md) — Scope, design or execution evidence; canonical owner T-0070.

Additional original references: `.meta/archive/tasks/0070-custom-ca-registry-initiative/custom-ca-registry-initiative.md`; `.meta/archive/tasks/0072-implement-the-accepted-opt-in-custom-ca-capability/quality-custom-ca-implementation-quality.md`.

## Provenance

Migrated from the T-0072 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
