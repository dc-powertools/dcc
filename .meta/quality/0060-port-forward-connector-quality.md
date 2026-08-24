# T-0060 Quality Record

- Date: 2026-08-24
- Change: Replace the direct container `nc` invocation with a baked, variant-aware
  connector and preserve response draining after request EOF.
- Route: Initiative
- Risk: Medium
- Owner or reviewer: T-0060 isolated implementer

## Scope And Criteria

- User-visible outcome: `forwardPorts` exchanges that wait for request EOF can return
  their response across supported Debian, Alpine, RHEL, and Fedora connector variants.
- In scope: baked connector selection, compatible build provisioning, host relay EOF
  ownership, Docker-free regression coverage, and architecture documentation.
- Non-goals: direct support for BusyBox/traditional netcat, public configuration changes,
  and replacement with a compiled connector.

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Host uses one fixed connector boundary. | Exact argv unit test and diff review. | `docker exec -i CONTAINER /usr/local/share/dcc/dcc-connect 127.0.0.1 PORT` is the only production path. | Pass |
| OpenBSD and Nmap variants receive correct arguments. | Executable-fake selection matrix. | OpenBSD receives `-N`; Nmap Ncat does not; OpenBSD wins precedence. | Pass |
| Unsupported variants cannot satisfy provisioning. | BusyBox, traditional, arbitrary generic, and Ncat-impostor cases. | Every unsupported case exits 127 with an actionable error. | Pass |
| Build provisioning reuses compatible tools and installs fallbacks otherwise. | Generated Dockerfile assertions. | The wrapper is checked before apt/apk/yum/dnf fallbacks and checked again afterward; no generic `command -v nc` build short circuit remains. | Pass |
| Request EOF reaches the connector while its response drains. | Real child-process/fake-Docker test plus in-memory relay test. | Dropping owned child stdin after client EOF allows `response-after-eof` to drain; both tests pass. | Pass |
| Generated wrapper is executable and follows Feature installation. | Tar mode and Dockerfile ordering assertions. | `.dcc-generated/dcc-connect` has mode 0755; Feature install precedes wrapper copy and compatibility checks. | Pass |
| Architecture matches implementation and fallback direction. | Documentation search and diff review. | Wrapper selection, package matrix, pipe ownership, and compiled-helper fallback are documented. | Pass |
| Existing Docker smoke passes in CI. | Ignored live-Docker smoke. | The unchanged smoke compiles, but this environment cannot run Docker. | Not run |

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | T-0060 r2 fixes the diagnosed EOF outcome while bounding unsupported variants. |
| Architecture and project context | Yes | The existing generated-asset and host relay boundaries support a small baked wrapper without a new dependency. |
| Data, security, and permissions | Yes | Host and port are validated; selection uses direct `exec`, no `eval`; no host permissions or Docker privileges change. |
| Slices and ownership | Yes | Wrapper/relay, build provisioning, tests, and architecture form one reviewable compatibility outcome. |
| Verification and rollback | Yes | Deterministic Docker-free tests cover each boundary; reverting the task commit restores the prior connector. |

Readiness verdict: **Ready**.

## Verification Results

| Required? | Check Or Method | Observed Result | Status | Unblocking Condition If Not Run |
| --- | --- | --- | --- | --- |
| Yes | Focused `forward::tests` | 11 passed. | Pass | |
| Yes | Focused `features::context::tests` | 35 passed. | Pass | |
| Yes | `cargo fmt --check` | Passed. | Pass | |
| Yes | `cargo check` | Passed. | Pass | |
| Yes | `cargo clippy -- -D warnings` | Passed with no warnings. | Pass | |
| Yes | `cargo test` | 510 unit, 31 runnable CLI, 9 config, 13 fake-Docker boundary, and 9 Feature CLI tests passed; 3 CLI and 32 Docker tests remained ignored. | Pass | |
| Yes | `cargo build` | Passed. | Pass | |
| Yes | `git diff --check` and scoped review | Passed; no unrelated tracked files included. | Pass | |
| CI-owned | `forwarded_port_reaches_container_loopback_service` | Compiled unchanged; live execution unavailable because Docker cannot run here. | Not run | Run the existing ignored smoke in Docker-capable CI. |

- Criteria or methods amended after implementation began, with reason and impact: the
  real subprocess test exposed that `ChildStdin::shutdown` does not release the pipe
  handle. The relay now drops the owned handle immediately after request EOF; this is
  necessary for the same promised EOF outcome and does not expand public scope.
- Counterfactual evidence for new regression or behavior tests: before the pipe-ownership
  correction, the subprocess response-drain test timed out. The old direct-`nc` argv
  cannot satisfy the fixed-boundary assertion, and omitting OpenBSD `-N`, adding it to
  Ncat, or accepting arbitrary `nc` fails a distinct matrix assertion.
- Flaky result and disposition: none observed.

## Review Findings

| Severity | File Or Area | Finding | Required Fix | Status |
| --- | --- | --- | --- | --- |
| Medium | Child stdin ownership | Flushing without dropping leaves EOF unobservable to the subprocess while stdout waits. | Drop the owned `ChildStdin` after request copy completes. | Resolved |
| Low | Minimal images | An early wrapper draft depended on external `grep`. | Use POSIX shell pattern matching for standalone `-N` detection. | Resolved |

## Consistency

| Canonical Source | Expected | Actual | Required Update |
| --- | --- | --- | --- |
| User request | Include a workaround for common missing `-N`. | Variant-aware wrapper supports OpenBSD and Nmap, rejects unsupported families. | None |
| T-0060 r2 brief | Fixed wrapper, compatible provisioning, Docker-free evidence. | Implementation and focused matrix match. | None |
| Project standards | Mandatory Rust checks and task-scoped commit. | Runnable checks passed; commit follows after record closure. | None |
| Tests and docs | Stable boundary and EOF behavior agree. | Exact argv, real subprocess, build context, and architecture align. | None |
| State and assumptions | Docker remains CI-owned in this environment. | Live smoke is the only remaining verification condition. | Run CI smoke. |

## Batch And Residual Risk

- Large-diff split trigger hit: No. The relay, wrapper, provisioning, tests, and docs are
  one cross-layer port-forwarding contract and share one regression outcome.
- Risk not resolved by passing checks: fake connector identities cannot prove every
  distribution package layout, and no live Docker/image build can run in this environment.

## Completion

- Required runnable checks all passed: Yes
- Status: Needs verification
- Exact incomplete condition, if not Done: the existing live Docker port-forward smoke
  must pass in Docker-capable CI.
- Next action: run `forwarded_port_reaches_container_loopback_service` in CI, then mark
  T-0060 Done if it passes.
