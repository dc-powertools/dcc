# Custom-CA OCI Feature Transport Threat Model

## Scope

- Change: opt-in exact-authority custom roots plus a local TLS OCI package-to-image
  fixture for T-0070 through T-0073.
- Assets or data: Feature install scripts and metadata, bearer tokens, CA bundles,
  registry responses, generated Docker build context, built images, and CI host state.
- Users, systems, or agents involved: dcc user, devcontainer configuration author,
  private registry and token service, OCI redirect targets, Docker daemon, and CI runner.
- Trust boundaries: workspace config/files to host process; host filesystem to TLS root
  store; registry challenge/redirect input to outbound requests; downloaded archive to
  build context/root Feature execution; test fixture to Docker daemon and CI cleanup.

## What Can Go Wrong

| Threat | Impact | Likelihood | Existing Control | Gap |
| --- | --- | --- | --- | --- |
| Private CA is trusted for unrelated hosts | Interception of public registry, auth, or blob traffic | Medium | HTTPS and rustls public roots | One global reqwest client cannot express host-bound added roots. |
| Redirect downgrades to HTTP or sends a token cross-origin | Artifact or bearer-token disclosure | Medium | reqwest redirect behavior | Policy is implicit and not tested at the OCI boundary. |
| Registry delegates its CA to an arbitrary token realm | Broadened trust controlled by remote input | Medium | Realm must be HTTPS | Root selection is not yet authority-aware. |
| Malformed, empty, private-key, or unreadable PEM is ignored | False confidence or unsafe fallback pressure | Medium | None; custom CA absent today | Strict eager validation and contextual errors are required. |
| CA path substitution or diagnostics leak host/secrets | Local path confusion or sensitive output | Low | Errors generally omit bodies/tokens | New path input needs bounded semantics and sanitization tests. |
| Wrong root or hostname is accepted | Registry impersonation | Low/High impact | rustls verification | Negative TLS controls must prove both checks remain active. |
| Registry body/token appears in an error or debug log | Credential or service-data disclosure | Medium | Current tests sanitize response bodies and do not log tokens | Preserve under redirect/TLS errors and fixture assertions. |
| Fixture key is committed, reused, or left running | Secret-like material/reachable service and CI contamination | Low | Existing ignored Docker tests use scoped cleanup | Generate ephemeral key/cert, bind loopback/random port, and use unconditional cleanup. |
| Fixture test passes without production configuration | False closure evidence | Medium | None | Smoke must invoke compiled CLI with `registryCAs`, and wrong/missing CA controls must fail. |
| Feature archive escapes or runs unexpected fixture content | Host/build compromise | Low | digest verification, safe archive paths/types, controlled Docker build | Fixture must be minimal and retain `./` plus archive negative tests. |

## Mitigations

| Mitigation | Owner | Verification | Status |
| --- | --- | --- | --- |
| Canonical exact-authority map and per-authority reqwest clients | T-0072 | Unit parsing/merge tests and multi-authority TLS routing test | Planned |
| Keep built-in roots; no HTTP or insecure retry in production | T-0072 | Default/public path test plus downgrade/wrong-root/hostname negatives | Planned |
| Disable implicit redirects; bound hops and strip authorization cross-origin | T-0072 | Same-origin, cross-origin, downgrade, loop, and hop-limit contract tests | Planned |
| Select token-realm trust by realm authority only | T-0072 | Split-host realm fails until its authority is configured | Planned |
| Eager strict PEM/path validation without variable expansion | T-0072 | Missing, directory, unreadable, empty, malformed, key-only, mixed-data tests | Planned |
| Preserve sanitized diagnostics and scoped token cache | T-0072 | Secret sentinel assertions in errors/log capture and request recording | Planned |
| Generate ephemeral localhost CA/key/cert and contain fixture | T-0073 | No static private key search; loopback bind; cleanup assertion | Planned |
| Exercise compiled `dcc build`, `./` archive entry, digest, install marker, and image run | T-0073 | Explicit ignored Docker smoke in serial CI | Planned |
| RAII/process cleanup for registry, containers, network, temp files, and images | T-0073 | Success and forced-failure cleanup branches; post-test Docker assertions | Planned |

## Agentic Risks

- Untrusted instructions or prompt injection: registry content and Feature scripts are
  untrusted data/code, never agent instructions; no agent tool consumes them as policy.
- Tool permission risk: the ignored smoke can create Docker containers, networks, and
  images only under unique test labels/names; cleanup must target those exact resources.
- Dependency, script, or generated-code risk: test-only TLS crates increase supply-chain
  surface; pin through `Cargo.lock`, use no downloaded executable, and review advisories.
- Secret or sensitive-data exposure risk: certificates may be logged only by authority;
  ephemeral private keys stay in a temporary directory and are never printed or committed.
- CI/CD or deployment permission risk: the smoke runs only in the existing serial Docker
  job with no publish credentials and no external registry.

## Residual Risk

- Accepted risk: a configured private CA can authenticate any certificate it signs for
  that exact authority, and malicious Feature install scripts execute as root during the
  Docker build; both are explicit user trust decisions inherent in the feature model.
- Approval or decision record: `.meta/decisions/0007-registry-scoped-custom-ca.md`.
- Review trigger: credentials, HTTP registries, wildcard authority matching, external
  fixture services, or a change to reqwest/rustls trust and redirect behavior.
