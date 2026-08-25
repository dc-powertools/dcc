# Custom Registry CA And TLS Feature Smoke Initiative

## Identity And Source

- Parent task: T-0070
- Revision: r1
- Authority: Product-owner request, 2026-08-25
- Catalog: `.meta/tasks/README.md`
- Child tasks: T-0071 through T-0073

## Goal

`dcc` supports an explicit, documented custom certificate-authority configuration for
private OCI Feature registries, and a deterministic ignored Docker smoke test proves a
minimally packaged Feature can travel through a TLS registry, `dcc build`, and image
execution without weakening default HTTPS verification.

## Scope

In scope:

- Design the user-facing custom-CA contract, trust scope, precedence, validation,
  diagnostics, and compatibility behavior.
- Implement opt-in custom CA loading in the production OCI client while retaining the
  built-in public trust store and HTTPS requirement.
- Build a minimal Dev Container Feature archive containing the regression-relevant
  `./` directory entry, valid metadata, and an install marker.
- Serve or publish that package through an ephemeral TLS-enabled OCI registry fixture.
- Drive the compiled `dcc build` command against the fixture, run the resulting image,
  verify the marker, and clean every registry/container/image resource.
- Keep ordinary `cargo test` Docker- and network-free; run the ignored smoke in the
  existing serial Docker CI job.
- Document private-registry CA setup, security boundaries, troubleshooting, and test
  architecture.

Out of scope:

- Disabling certificate or hostname verification.
- Permitting plaintext HTTP registries or token realms in production.
- Changing Docker daemon trust configuration or the host's global CA store.
- Adding registry credentials, general-purpose OCI publishing commands, or support for
  non-Feature artifacts unless separately authorized.

## Required Design Decisions

- Configuration surface: CLI flag, dcc configuration, environment variable, or a
  justified combination.
- Trust binding: global additional roots versus registry-host-bound roots; prefer the
  narrowest usable scope.
- Path and PEM semantics, multiple certificates, precedence, duplicate handling, and
  behavior when the file is absent, unreadable, malformed, or contains no certificates.
- Whether redirects and token realms may use the added root, and how host/scheme changes
  are constrained.
- How the integration fixture provides TLS and OCI responses without embedding secrets,
  modifying system trust, or depending on an external mutable registry.

## Initiative Acceptance

- Default users retain the current public-root HTTPS behavior byte-for-byte at the
  configuration boundary; no insecure fallback exists.
- A configured private CA authorizes only the accepted, documented registry trust
  boundary and produces contextual, non-secret errors when invalid.
- Unit and contract tests cover public defaults, custom-root success, wrong-root and
  hostname failures, malformed inputs, redirect/token-realm policy, and option
  precedence.
- An ignored Docker smoke creates a minimal package with `./`, serves it through local
  TLS, runs the compiled `dcc build`, verifies the installed image marker, and proves
  cleanup. It is invoked explicitly by Docker CI.
- User documentation, architecture, threat model, task/quality records, and command
  catalog are aligned; every required runnable gate passes.

## Task Decomposition

| Task | Boundary | Done When |
| --- | --- | --- |
| T-0071 | Product/security design | A decision record, threat model, fixture design, exact acceptance matrix, and implementation plan are approved and implementation-ready. |
| T-0072 | Production capability | The selected custom-CA contract is implemented with deterministic non-Docker tests and documentation, without insecure fallback. |
| T-0073 | Package-to-image smoke and closure evidence | The TLS OCI fixture and ignored Docker smoke exercise the compiled CLI through marker verification and cleanup; CI, full verification, and security review pass with evidence ready for parent reconciliation. |

## Risk And Verification

- Risk: High. This changes a TLS trust boundary and adds CI infrastructure that handles
  certificates and untrusted archive/network input.
- Rollback: Revert the task-scoped implementation commits; no migration or persisted
  production state is required.
- Required gates: Decision and threat-model review; certificate/hostname negative
  controls; archive safety tests; format/check/all-target Clippy/full tests/build;
  ignored Docker smoke; workflow lint; security, documentation, cleanup, and diff
  review.
