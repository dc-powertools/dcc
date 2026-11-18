# T-0071: Decide the production custom-CA configuration and security contract, including registry binding, precedence, PEM/path validation, redirect and token-realm behavior, diagnostics, and the deterministic TLS OCI fixture design.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0071

## Outcome

Decide the production custom-CA configuration and security contract, including registry binding, precedence, PEM/path validation, redirect and token-realm behavior, diagnostics, and the deterministic TLS OCI fixture design.

## Authority / Revision

Parent T-0070; user request 2026-08-25 / r1

## Status

Done

## Depends On

None

## Route / Risk

Decide / High

## Approval Or Blocker

None

## Next Safe Action

Stop; accepted design and handoff are complete.

## Result

Accepted exact-authority custom-CA contract, updated OCI threat model, dependency and fixture design, and exhaustive production/TLS/Docker test matrices after independent architecture and security review; artifact consistency and diff checks passed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [decision 0007 registry scoped custom ca](decision-0007-registry-scoped-custom-ca.md) — Historical decision rationale.
- [quality custom ca design quality](quality-custom-ca-design-quality.md) — Verification and review evidence.
- [custom ca registry initiative](../0070-custom-ca-registry-initiative/custom-ca-registry-initiative.md) — Scope, design or execution evidence; canonical owner T-0070.
- [threat custom ca oci](../0070-custom-ca-registry-initiative/threat-custom-ca-oci.md) — Security and trust-boundary analysis; canonical owner T-0070.

Additional original references: `.meta/archive/tasks/0070-custom-ca-registry-initiative/custom-ca-registry-initiative.md`; `.meta/archive/tasks/0071-decide-the-production-custom-ca-configuration-and-security/decision-0007-registry-scoped-custom-ca.md`; `.meta/archive/tasks/0070-custom-ca-registry-initiative/threat-custom-ca-oci.md`; `.meta/archive/tasks/0071-decide-the-production-custom-ca-configuration-and-security/quality-custom-ca-design-quality.md`.

## Provenance

Migrated from the T-0071 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
