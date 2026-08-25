# T-0073 TLS OCI Package-To-Image Smoke Quality Record

- Date: 2026-08-25
- Change: ephemeral TLS OCI Feature package-to-image smoke and serial Docker CI coverage
- Route: Initiative
- Risk: High
- Owner or reviewer: T-0073 orchestrator with delegated fixture/CI design, security/diff review, and independent QA

## Scope And Criteria

- User-visible outcome: CI proves that the compiled `dcc build` can download a minimal
  Feature from a private-CA localhost registry, install it into an image, and run its marker.
- In scope: generated TLS material, loopback OCI fixture, explicit `./` package entry,
  digest/request contracts, missing/wrong-CA controls, exact cleanup, CI, and test docs.
- Non-goals: production trust changes, registry credentials, an external OCI Feature registry,
  HTTP or insecure TLS, host/Docker trust mutation, and ordinary-test Docker/network use.

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Fixture is ephemeral and contained | Source review plus focused failure-path run | Post-T-0080 rerun passed: CA and leaf generated per run; key remains in memory; public CA files, loopback listener, and temporary workspace disappear | Pass |
| OCI package and responses are exact | Package contract, digest unit tests, request capture | Post-T-0080 package contract passed; raw `./` first entry, metadata, executable installer, SHA-256 manifest/blob path, and bounded three-GET sequence remain covered | Pass |
| Wrong trust fails before Docker | Compiled CLI negative control | Post-T-0080 rerun passed: missing/wrong CA preserve certificate context, record no HTTP, never reach fake Docker, and clean the fixture | Pass |
| Trusted package reaches the image | Ignored Docker smoke | CI merge `e892a552` passed the trusted TLS OCI build through production `registryCAs` | Pass |
| Built image marker is verified | Ignored Docker smoke | CI passed the exact named/labeled marker run | Pass |
| Cleanup covers success and failure | Failure-path run, resource-guard review, live assertions | CI passed exact success/failure cleanup assertions; server and temporary cleanup also passed | Pass |
| CI and docs make the smoke durable | Workflow lint and doc review | Dedicated target is invoked explicitly with one test thread in the existing Docker job | Pass |

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | T-0070 brief and T-0071 matrix define this child boundary. |
| Architecture and project context | Yes | Existing compiled-CLI integration and serial Docker-CI seams are reused. |
| Data, security, and permissions | Yes | Threat model 0070 prohibits static keys, external state, insecure trust, and broad cleanup. |
| Slices and ownership | Yes | Dedicated integration target isolates fixture/resource lifecycle; root retains catalog/cursor. |
| Verification and rollback | Yes | User-provided CI logs supply the required image build/run/cleanup evidence. |

Readiness verdict: Ready; implementation and required evidence are complete.

## Verification Results

| Required? | Check Or Method | Observed Result | Status | Unblocking Condition If Not Run |
| --- | --- | --- | --- | --- |
| Yes | Package contract test | Post-T-0080 exact rerun passed 1 test; explicit root, metadata, and executable installer | Pass | |
| Yes | Ignored missing/wrong-CA control | Post-T-0080 exact serial rerun passed 1 test; certificate-context failures, zero HTTP records, fake Docker absent, listener/temp cleanup | Pass | |
| Yes | Existing archive/digest negatives | 7 extraction and 2 digest tests passed, including traversal/type/root and mutation failures | Pass | |
| Yes | Live ignored trusted smoke | User-provided `logs_88985560711.zip` records both ignored tests passing on Docker 28.0.4 at merge `e892a552`: missing/wrong CA, trusted build, marker run, forced-error cleanup, and exact absence assertions | Pass | |
| Yes | `cargo fmt --check` | Passed | Pass | |
| Yes | `cargo check --locked` | Passed | Pass | |
| Yes | `cargo clippy --all-targets -- -D warnings` | Passed | Pass | |
| Yes | `cargo test --locked` | T-0080 replaced the released-port rebind race with an exact-object lifetime oracle; its clean first run under concurrent port churn passed 555 unit and 68 runnable integration tests | Pass | |
| Yes | `cargo build --locked` | Passed | Pass | |
| Yes | `actionlint .github/workflows/*.yml` | Passed with checksum-verified actionlint 1.7.12 for the committed workflow; unavailable in the resumed shell, and T-0080 did not change the workflow | Pass | |
| Yes | Release workflow contract and diff check | Both passed | Pass | |
| Yes | Dependency/static security review | No manifest/lock delta; one rustls line; no static key, insecure mode, external Feature registry, or trust mutation | Pass | |
| Yes | Independent security/diff and QA review | Post-T-0080 reviews and CI log audit found no product/test defect. The audit exposed broad write token permissions; T-0081 remediated them with workflow-level `contents: read` and non-persisted checkout credentials | Pass | |

- Criteria or methods amended after implementation began, with reason and impact: split the
  Docker-free TLS negatives from the trusted live smoke so the security boundary remains
  independently runnable; added an exact Docker resource guard and forced-error probe after
  design review. This strengthened cleanup evidence without changing scope.
- Counterfactual evidence: omitted and wrong CA both reach the same generated server, preserve
  certificate-verification context, record no HTTP request, and do not invoke fake Docker.
- Flaky result and disposition: the earlier T-0075 released-address rebind signal was corrected
  by T-0080, which now observes ownership of the exact listener objects without competing for
  their released ports. Its retained-owner counterfactual failed as intended, 200 concurrent
  stress runs passed, and the first full-suite run under port churn passed cleanly. The signal no
  longer blocks T-0073.

## Review Findings

| Severity | File Or Area | Finding | Required Fix | Status |
| --- | --- | --- | --- | --- |
| Medium | Initial fixture placement | Existing Docker smoke file was already large and used a different lifecycle | Move to a dedicated integration target | Resolved |
| High | Initial cleanup design | Fake-Docker failure created no resource and unnamed marker could evade cleanup | Add exact registered resources, named/labeled marker, and real forced-error probe | Resolved |
| Medium | Initial fixture logging/key handling | Request log was unbounded and stored header values; leaf key was written to disk | Bound records, store authorization presence only, keep key in memory | Resolved |
| Medium | Initial wrong-CA config | Escaped newlines caused a parse failure that could masquerade as a trust failure | Require authority plus certificate context; write actual JSON newlines | Resolved |
| Low | Test documentation | “No external registry” overstated isolation because Docker may pull the base image | Say no external OCI Feature registry and disclose the base-image pull | Resolved |
| High | CI token boundary | The passing PR job received broad write scopes and persisted checkout credentials | Restrict CI to `contents: read` and set every checkout to `persist-credentials: false` | Resolved by T-0081 |

## Consistency

| Canonical Source | Expected | Actual | Required Update |
| --- | --- | --- | --- |
| T-0070 brief | TLS OCI package reaches image marker without weaker trust | CI passed the dedicated negative controls, trusted build, marker, and cleanup | None |
| Decision 0007 | Exact `registryCAs`, HTTPS, no fallback | Compiled CLI uses declaration-relative CA; no alternate trust path | None |
| Threat model 0070 | Ephemeral keys, loopback, bounded input, exact cleanup | Implemented and status table aligned | None |
| Tests and docs | Ordinary tests Docker-free; ignored smoke explicit in serial CI | Dedicated target and maintainer/architecture docs align | None |
| State | T-0073 closes after every required gate and remediation passes | CI evidence passes; token boundary remediated by T-0081 | Root transition to Done |

## Batch And Residual Risk

- Large-diff split trigger hit: Yes.
- If kept together, why: fixture, package, request assertions, compiled CLI, resource cleanup,
  workflow invocation, and test architecture form one independently executable end-to-end proof;
  the dedicated test target isolates them from production and existing Docker tests.
- Risk not resolved by passing checks: live behavior depends on Docker, base-image pull
  availability, and CI runner networking; Feature install scripts inherently execute as root.

## Completion

- Required checks all passed: Yes.
- Status: Done
- Exact incomplete condition, if not Done: None.
- Next action: Root closes T-0073 and reconciles T-0070.
