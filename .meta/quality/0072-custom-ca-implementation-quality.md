# T-0072 Registry Custom-CA Implementation Quality Record

- Date: 2026-08-25
- Change: exact-authority custom CA support in the production OCI Feature client
- Route: Initiative
- Risk: High
- Owner or reviewer: T-0072 orchestrator with delegated config, OCI, QA, and cross-review agents

## Scope And Criteria

- User-visible outcome: private HTTPS OCI Feature registries and their token services
  can use explicitly configured CA bundles without weakening public-root defaults.
- In scope: config schema, inheritance and path provenance, authority and PEM validation,
  exact trust selection, redirects, bearer realms, diagnostics, Docker-free TLS tests,
  dependencies, and user/architecture docs.
- Non-goals: HTTP/insecure mode, wildcard trust, credentials, host trust-store mutation,
  or the Docker package-to-image smoke owned by T-0073.

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Config and inheritance follow decision 0007 | Parser, merge, and load tests | Duplicate-aware canonical map; declaring-file paths; canonical child override; eager surviving-entry validation | Pass |
| CA input is bounded and strict | File/PEM negative matrix | Regular/readable file, 1 MiB bound, cert-only framing, base64/DER/root validation, dedupe, no expansion | Pass |
| Trust stays exact and additive | Generated TLS counterfactuals and static review | Missing/wrong/expired/hostname/unconfigured targets fail; configured target succeeds; built-ins never disabled | Pass |
| Redirect and bearer credentials stay scoped | HTTP/TLS request capture | Ten-hop bound, target trust reselection, same-origin retention, cross-origin stripping, separate realm CA | Pass |
| Diagnostics do not expose sensitive material | Sentinel tests and error review | PEM, token, response body, userinfo, query, and Location sentinels absent | Pass |
| Public behavior and docs remain coherent | Full suite, dependency and docs review | Unconfigured path uses redirect-disabled public client and no CA reads; docs describe exact contract | Pass |

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | T-0070 brief and accepted decision 0007 define this child boundary. |
| Architecture and project context | Yes | T-0071 quality matrix identified config, transport, dependency, and fixture seams. |
| Data, security, and permissions | Yes | Threat model 0070 covers trust roots, tokens, redirects, archives, and diagnostics. |
| Slices and ownership | Yes | Config/dependency and OCI/security files had non-overlapping delegated owners; orchestrator integrated docs/evidence. |
| Verification and rollback | Yes | Deterministic negative/positive TLS controls plus full gates; rollback is the task-scoped commit revert. |

Readiness verdict: Ready

## Verification Results

| Required? | Check Or Method | Observed Result | Status | Unblocking Condition If Not Run |
| --- | --- | --- | --- | --- |
| Yes | `cargo fmt --check` | Passed | Pass | |
| Yes | `cargo check` | Passed | Pass | |
| Yes | `cargo clippy --all-targets -- -D warnings` | Passed | Pass | |
| Yes | Focused authority/PEM/provenance tests | 13 registry-CA tests, 4 extends/eager tests, and canonical merge test passed | Pass | |
| Yes | Focused Feature/OCI/TLS tests | 35 OCI tests and 63 Feature boundary tests passed; generated TLS matrix included private success and negative controls | Pass | |
| Yes | `cargo test --locked` | 555 unit tests and 67 runnable integration tests passed; 35 environment-dependent tests ignored | Pass | |
| Yes | `cargo build` | Passed | Pass | |
| Yes | Dependency review | One rustls 0.23/tokio-rustls 0.26 line; rustls-pemfile 2.2; rcgen 0.14 dev-only; compatible licenses | Pass | |
| Yes | Static bypass and diff checks | No invalid-cert/hostname bypass, built-in-root disablement, production HTTP constructor, or whitespace errors | Pass | |

- Criteria or methods amended after implementation began, with reason and impact:
  Feature-provided `registryCAs: null`, exact-1-MiB input, not-yet-valid leaf,
  token-redirect, and additive-root coverage were added during security review; they
  strengthen the accepted boundary without changing behavior.
- Counterfactual evidence: the same generated TLS fixtures failed when the CA entry was
  absent, wrong, expired, hostname-invalid, on another port, or missing for a split realm;
  exact entries then succeeded.
- Flaky result and disposition: None.

## Review Findings

| Severity | File Or Area | Finding | Required Fix | Status |
| --- | --- | --- | --- | --- |
| High | OCI trust clients | Eager/global root clients could broaden trust or violate the accepted lazy boundary | Build and cache a client only on first exact-authority use | Resolved |
| High | Redirects and realms | Any implicit redirect path could retain credentials or reuse registry trust | Disable reqwest redirects for every client and route all GETs through one bounded executor | Resolved |
| Medium | Authority diagnostics | Raw invalid authorities could expose URL user information or query values | Use sanitized validation errors and sentinel tests | Resolved |
| Medium | Feature metadata | Optional-value parsing could miss an explicit null trust declaration | Detect key presence with flattened metadata and reject any value | Resolved |
| Medium | TLS diagnostics | Formatting reqwest failures could flatten the certificate error source chain | Preserve the URL-sanitized reqwest error as the anyhow source | Resolved |
| Medium | Feature boundary | Outer download context echoed the raw, potentially credential-bearing Feature reference | Use generic boundary context and a production-path sentinel test | Resolved |

## Consistency

| Canonical Source | Expected | Actual | Required Update |
| --- | --- | --- | --- |
| User request / T-0070 | Supported private-registry CA trust without weaker HTTPS | Exact additive authority roots; no fallback | None |
| Decision 0007 | Canonical map, declaration provenance, strict eager PEM, manual redirects/realm | Implemented and covered by generated TLS and negative tests | None |
| Threat model 0070 | Roots and tokens do not cross authority boundaries | Per-hop client selection and credential capture prove isolation | None |
| Tests and docs | Docker-free T-0072 matrix and accurate user/architecture guidance | Focused/full suites and docs align | None |
| Task state | T-0072 implementation only | T-0073 remains the dependent Docker smoke | Root catalog/cursor transition required |

## Batch And Residual Risk

- Large-diff split trigger hit: Yes.
- If kept together, why: config provenance, transport selection, redirect/auth behavior,
  and their live TLS controls form one security boundary and were independently delegated
  before integration.
- Risk not resolved by passing checks: deterministic tests model additive public defaults
  without contacting a live WebPKI service; a configured CA can authorize any valid leaf
  for its exact authority; unusual enterprise PEM encodings outside strict certificate-only
  bundles are intentionally rejected; the new `rcgen` test dependency requires Rust 1.88+
  (the repository declares no lower MSRV); T-0073 still owns the live Docker package-to-image
  path.

## Completion

- Required checks all passed: Yes.
- Status: Done
- Exact incomplete condition, if not Done: None.
- Next action: Root marks T-0072 Done and T-0073 Ready.
