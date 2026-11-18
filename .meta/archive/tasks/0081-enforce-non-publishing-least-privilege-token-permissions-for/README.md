# T-0081: Enforce non-publishing least-privilege token permissions for every CI job, including direct and reusable entrypoints, and align the custom-CA CI threat boundary with observed behavior.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0081

## Outcome

Enforce non-publishing least-privilege token permissions for every CI job, including direct and reusable entrypoints, and align the custom-CA CI threat boundary with observed behavior.

## Authority / Revision

Required security correction discovered while verifying parent T-0073 from user-provided CI logs 2026-08-25 / r1

## Status

Done

## Depends On

None

## Route / Risk

Quick change / High

## Approval Or Blocker

None

## Next Safe Action

Stop; remediation complete.

## Result

Added workflow-level `contents: read` and set all five CI checkout steps to `persist-credentials: false`. Checksum-verified actionlint 1.7.12, the existing release-workflow contract, exact configuration inspection, and diff checks passed. No release caller or publishing workflow was changed.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [threat custom ca oci](../0070-custom-ca-registry-initiative/threat-custom-ca-oci.md) — Security and trust-boundary analysis; canonical owner T-0070.
- [quality tls oci docker smoke quality](../0073-implement-and-ci-enable-an-ignored-end-to/quality-tls-oci-docker-smoke-quality.md) — Verification and review evidence; canonical owner T-0073.

Additional original references: `.meta/archive/tasks/0070-custom-ca-registry-initiative/threat-custom-ca-oci.md`; `.meta/archive/tasks/0073-implement-and-ci-enable-an-ignored-end-to/quality-tls-oci-docker-smoke-quality.md`.

## Provenance

Migrated from the T-0081 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
