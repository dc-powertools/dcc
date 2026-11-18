# T-0073: Implement and CI-enable an ignored end-to-end smoke that packages a minimal Feature containing `./`, exposes it through ephemeral trusted TLS OCI infrastructure, runs the compiled `dcc build`, verifies the image marker, guarantees cleanup, and supplies closure evidence for T-0070.

This README owns current task state. Supporting files preserve evidence and historical checkpoints; earlier instructions in them do not supersede this record or current project/framework requirements.

## Identity

T-0073

## Outcome

Implement and CI-enable an ignored end-to-end smoke that packages a minimal Feature containing `./`, exposes it through ephemeral trusted TLS OCI infrastructure, runs the compiled `dcc build`, verifies the image marker, guarantees cleanup, and supplies closure evidence for T-0070.

## Authority / Revision

Parent T-0070 / r1

## Status

Done

## Depends On

T-0072, T-0081

## Route / Risk

Initiative / High

## Approval Or Blocker

None

## Next Safe Action

Stop; outcome complete.

## Result

CI merge `e892a552` passed the missing/wrong-CA control and trusted TLS OCI build, marker, forced-error cleanup, and exact post-cleanup assertions; all other CI jobs passed without annotations. T-0081 remediated the observed write-token exposure with read-only permissions and non-persisted checkout credentials.

## Scope and acceptance

The outcome above and the linked scope/acceptance in supporting evidence define this task. For catalog-only completed tasks, the preserved outcome and result are the available acceptance/completion evidence; this migration invents no additional checks or claims.

## Supporting evidence

- [quality tls oci docker smoke quality](quality-tls-oci-docker-smoke-quality.md) — Verification and review evidence.
- [custom ca registry initiative](../0070-custom-ca-registry-initiative/custom-ca-registry-initiative.md) — Scope, design or execution evidence; canonical owner T-0070.

Additional original references: `.meta/archive/tasks/0070-custom-ca-registry-initiative/custom-ca-registry-initiative.md`; `.meta/archive/tasks/0073-implement-and-ci-enable-an-ignored-end-to/quality-tls-oci-docker-smoke-quality.md`.

## Provenance

Migrated from the T-0073 catalog row at Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Original identity, authority/revision, dependencies, limitations and results are preserved; historical status remains historical evidence rather than a new verification.
