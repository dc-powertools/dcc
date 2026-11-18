# T-0071 Custom-CA Design Quality Record

- Date: 2026-08-25
- Change: custom-CA security contract and deterministic TLS OCI fixture design
- Route: Decide
- Risk: High
- Owner or reviewer: T-0071 orchestrator with independent architecture and security/QA review

## Scope And Criteria

- User-visible outcome: T-0072 and T-0073 can implement private-registry CA support
  without inventing trust, precedence, validation, redirect, diagnostic, or fixture rules.
- In scope: accepted decision, threat model, behavior/test matrix, dependency assessment,
  fixture design, rollback, and child-task handoff.
- Non-goals: production or fixture code, credentials, HTTP/insecure mode, or global trust changes.

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Configuration and precedence are exact | Read-through against config merge/resolve | Authority map, declaration-relative paths, canonical child override, no alternate input | Pass |
| Trust is authority-bound | Adversarial transport review | Per-target clients, manual HTTPS redirects, exact realm lookup, cross-origin credential stripping | Pass |
| PEM/path failures are deterministic | Negative-case matrix review | File/size/read, PEM object/count/DER, wrong-root, and hostname cases enumerated | Pass |
| Fixture is deterministic and contained | Existing OCI/Docker test review | Ephemeral rustls server, loopback, generated certs, local OCI, compiled CLI, marker, exact cleanup | Pass |
| Dependencies are assessed | Manifest/lock inspection | Production `rustls-pemfile`; test-only `rcgen`, `rustls`, `tokio-rustls`; no OpenSSL process | Pass |
| Independent high-risk review occurs | Architecture and security/QA worker review | Both completed; findings incorporated below | Pass |

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | T-0070 goal, non-goals, and child boundaries remain unchanged. |
| Architecture and project context | Yes | Decision 0007 defines config, transport, dependencies, diagnostics, and compatibility. |
| Data, security, and permissions | Yes | Threat model 0070 covers roots, tokens, redirects, archives, Docker, and CI. |
| Slices and ownership | Yes | T-0072 owns production/non-Docker TLS; T-0073 owns Docker fixture/CI. |
| Verification and rollback | Yes | Matrices define counterfactuals, full gates, cleanup, and revert-only rollback. |

Readiness verdict: Ready

## Exact T-0072 Behavior And Test Matrix

| Area | Required behavior and verification |
| --- | --- |
| Default | Missing/empty map uses public roots and HTTPS only and reads no CA file. Public TLS succeeds; private TLS fails without config. |
| Schema | `registryCAs` maps exact authority to one PEM-bundle path. Strict mode accepts it; non-string values fail. Feature metadata cannot contribute it. |
| Authority | Lowercase DNS; omitted port equals `:443`; bracket IPv6; reject scheme, userinfo, path, query, fragment, space, trailing dot, port 0. Reject canonical collisions; different port does not match. |
| Extends | Resolve a relative path at its declaring file before map merge. Child canonical authority replaces parent; unrelated authorities union. |
| File/PEM | Eager regular readable file <=1 MiB with one or more valid certificates. Test missing, directory, unreadable where supported, oversized, empty, key-only, malformed/truncated/base64-invalid/invalid-DER, trailing junk, and mixed objects. |
| Bundle | Multiple roots accepted and byte-identical roots idempotent. Test success when the signer is not first and when repeated. |
| TLS scope | Configured roots augment public roots for the exact target. Test private success; wrong root, hostname, validity window, same CA on unconfigured authority failure; public chain still succeeds. |
| Redirect | Manual maximum 10; relative/same-origin succeeds; each target is HTTPS and independently trusted. Cover manifest/blob redirect, target-specific trust, cross-origin Authorization absence, downgrade, loop, missing/bad Location, and hop 11. |
| Bearer | Absolute HTTPS realm uses its own exact authority entry, never implicit registry delegation. Cover same/split authority, missing auth-host entry, HTTP rejection, and token redirects. |
| Credentials/cache | Authorization survives same-origin only; registry token never reaches realm or cross-origin. Cache remains normalized registry+scope; record headers for multiple authorities/scopes. |
| Diagnostics | Include operation/authority and local path for read failures only. Secret sentinels for PEM, token, response body, userinfo, and query stay absent from errors/debug. |
| Static safety | Diff contains no invalid-certificate/hostname bypass or production HTTP escape; existing HTTP fixture constructor stays test-only. |

Counterfactual: use the same live server and show failure when the CA entry is removed,
replaced by a wrong root, or addressed with a nonmatching hostname. Trust-bleed proof
uses one signer for two authorities and configures only one.

## Exact T-0073 Fixture And Smoke Matrix

| Area | Required design and proof |
| --- | --- |
| TLS | Generate ephemeral CA and localhost leaf with DNS/IP SANs in a temp directory; bind `127.0.0.1:0`; serve through rustls; no static key or system trust mutation. |
| OCI | Script `/v2/`, one OCI manifest, and a digest-matched Feature blob; capture bounded method/path/header records. |
| Package | Tar explicitly includes directory `./`, metadata, and executable `install.sh` writing a fixed image marker. |
| CLI/outcome | Temporary profile references `localhost:<port>/owner/feature:1` and its CA. Invoke compiled `dcc build`, run the image, assert marker and requested paths/digest. |
| Negative | Omitted/wrong CA fails against the TLS fixture before Docker build; retain archive traversal/type negatives. |
| Cleanup | Unique exact names/labels; RAII fallback plus explicit success/failure cleanup for server, temp files, containers/network if used, and image; query exact resources to prove absence. |
| CI | Ordinary tests remain Docker/network-free. Explicitly run the ignored smoke in existing serial Docker CI with `--test-threads=1`; lint workflow. |

Prefer direct dev dependencies `rcgen`, `rustls`, and `tokio-rustls` over an OpenSSL
executable or external registry. Confirm locked versions, MSRV, licenses, `cargo tree`,
and lockfile delta. Production adds direct `rustls-pemfile` for strict object parsing.

## Verification Results

| Required? | Check Or Method | Observed Result | Status | Unblocking Condition If Not Run |
| --- | --- | --- | --- | --- |
| Yes | Decision/brief/threat-model consistency | Exact-authority additive trust and child handoffs agree | Pass | |
| Yes | Source/config/Cargo/workflow inspection | Located global client, implicit redirects, merge/resolve boundary, Docker smoke boundary, locked TLS stack | Pass | |
| Yes | Independent architecture review | Completed; declarative config, per-client roots, strict PEM parsing, and fixture dependencies supported | Pass | |
| Yes | Independent security/QA review | Completed; redirect, provenance, realm, negative controls, and cleanup refinements incorporated | Pass | |
| Yes | Markdown/diff/whitespace checks | Full artifact read-through and `git diff --check` passed; record is within its 220-line budget | Pass | |

- Criteria amended after implementation began: Not applicable; this is the
  pre-implementation gate.
- Counterfactual evidence: prescribed above for T-0072/T-0073; T-0071 changes no behavior.
- Flaky result and disposition: None.

## Review Findings

| Severity | File Or Area | Finding | Required Fix | Status |
| --- | --- | --- | --- | --- |
| High | OCI client | One global client would broaden private trust | Select redirect-disabled client by exact target authority | Resolved |
| High | Redirects | Implicit following obscures trust and credential transitions | Bounded HTTPS loop; strip cross-origin authorization | Resolved |
| Medium | Token realm | Registry CA applied to another realm delegates trust from remote input | Require explicit realm-authority entry | Resolved |
| Medium | Extends | Leaf-relative path changes inherited meaning | Resolve at declaration before merge | Resolved |
| Medium | Fixture | Static keys/external service weaken containment | Ephemeral loopback TLS and exact cleanup assertions | Resolved |

## Consistency

| Canonical Source | Expected | Actual | Required Update |
| --- | --- | --- | --- |
| User request / T-0070 | Custom CA without weakening default HTTPS | Exact-authority additive trust; no insecure fallback | None |
| Task brief | Binding, precedence, validation, redirects/realm, diagnostics, fixture | Decision, threat model, and matrices cover all | None |
| Decisions/standards | High-risk record, rustls portability, no secrets | Aligned; dependency deltas explicit | None |
| Tests/docs | Implementation-ready behavior | Dependent handoffs enumerate tests and documentation | Implement in T-0072/T-0073 |
| State | T-0071 design only | No production behavior claimed | Root catalog close needed |

## Batch And Residual Risk

- Large-diff split trigger hit: No; all artifacts describe one trust-boundary decision.
- Risk not resolved by passing checks: unusual enterprise PEM encodings may fail strict
  parsing; a compromised configured CA controls its exact authority; Docker smoke needs
  its base image; root-running Feature scripts remain an explicit inherent trust choice.

## Completion

- Required checks all passed: Yes.
- Status: Done
- Exact incomplete condition, if not Done: None.
- Next action: Root marks T-0071 Done and T-0072 Ready.
