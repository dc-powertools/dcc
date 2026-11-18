# T-0070: Coordinate the full design and implementation of supported custom-CA trust for private OCI Feature registries plus a TLS package-to-image Docker smoke, without weakening default HTTPS verification.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0070

## Outcome

Coordinate the full design and implementation of supported custom-CA trust for private OCI Feature registries plus a TLS package-to-image Docker smoke, without weakening default HTTPS verification.

## Authority / Revision

User request 2026-08-25 / r1

## Status

Done

## Depends On

T-0071, T-0072, T-0073

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; initiative outcome complete.

## Result

Delivered exact-authority custom-CA configuration and production OCI trust, strict validation and safe redirect/token behavior, deterministic TLS counterfactuals, and a serial package-to-image Docker smoke. CI merge `e892a552` passed all ordinary and Docker gates including marker/cleanup; T-0081 remediated the sole log-audit finding by restricting CI credentials.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [custom ca registry initiative](custom-ca-registry-initiative.md) — Scope, design or execution evidence.
- [threat custom ca oci](threat-custom-ca-oci.md) — Security and trust-boundary analysis.
- [decision 0007 registry scoped custom ca](../0071-decide-the-production-custom-ca-configuration-and-security/decision-0007-registry-scoped-custom-ca.md) — Historical decision rationale; canonical owner T-0071.
- [quality custom ca design quality](../0071-decide-the-production-custom-ca-configuration-and-security/quality-custom-ca-design-quality.md) — Verification and review evidence; canonical owner T-0071.
- [quality custom ca implementation quality](../0072-implement-the-accepted-opt-in-custom-ca-capability/quality-custom-ca-implementation-quality.md) — Verification and review evidence; canonical owner T-0072.
- [quality tls oci docker smoke quality](../0073-implement-and-ci-enable-an-ignored-end-to/quality-tls-oci-docker-smoke-quality.md) — Verification and review evidence; canonical owner T-0073.

Additional original references: `.meta/archive/tasks/0070-custom-ca-registry-initiative/custom-ca-registry-initiative.md`; `.meta/archive/tasks/0071-decide-the-production-custom-ca-configuration-and-security/decision-0007-registry-scoped-custom-ca.md`; `.meta/archive/tasks/0071-decide-the-production-custom-ca-configuration-and-security/quality-custom-ca-design-quality.md`; `.meta/archive/tasks/0072-implement-the-accepted-opt-in-custom-ca-capability/quality-custom-ca-implementation-quality.md`; `.meta/archive/tasks/0073-implement-and-ci-enable-an-ignored-end-to/quality-tls-oci-docker-smoke-quality.md`; `.meta/archive/tasks/0070-custom-ca-registry-initiative/threat-custom-ca-oci.md`.

## Provenance

Migrated from the T-0070 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
