# T-0043 Brief: Test Quality Corrections

## Identity And Source

- Task ID: T-0043
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User follow-up to the full-project test audit
- Source reference and date: Test-quality audit and follow-up, 2026-08-24
- Parent or split task IDs: Parent initiative; children T-0044 through T-0053

## Goal

The test suite gives maintainers credible evidence about `dcc` correctness, utility,
compatibility, and safety. Tests should fail when a meaningful contract regresses and
should not obstruct harmless refactoring by locking in transient symbols, formatting,
or historical cleanup details.

## Background

The audit found a broad and generally valuable suite, but also found security-sensitive
behavior protected by the wrong assertions, major user workflows with no behavioral
coverage, Docker smokes whose names overstate what they verify, and low-value tests tied
to prior implementation incidents. The suite passed locally, but 36 ignored tests rely
on Docker and one nominally Docker-free dry-run test panics in fixture cleanup when the
Docker binary is absent.

## Child Outcomes

| Task | Outcome |
| --- | --- |
| T-0044 | Feature install scripts do not leak secrets through unconditional tracing. |
| T-0045 | UID remapping is tested and implemented as Linux-only. |
| T-0046 | Anonymous and named Feature volumes follow Docker's mount contract. |
| T-0047 | Port forwarding has behavioral and failure-path coverage. |
| T-0048 | OCI Feature acquisition and metadata parsing are tested at the trust boundary. |
| T-0049 | CLI and Docker smokes directly prove the lifecycle properties they name. |
| T-0050 | CLI/version/build/resource decisions are verified at the Docker argv boundary. |
| T-0051 | `containerEnv` undefined-value behavior has one consistent contract. |
| T-0052 | Redundant and transient implementation assertions are removed or rewritten. |
| T-0053 | Missing property tests and Feature-editing behavior matrices are added. |

## Scope

In scope:

- The ten independently verifiable child outcomes above.
- Production corrections exposed by replacing a bad assertion with the intended stable
  contract.
- Counterfactual evidence for new regression tests where safe and practical.
- Documentation changes required when a public contract changes or is clarified.

Out of scope:

- Chasing a coverage percentage or preserving the current test count.
- Rewriting useful tests solely for stylistic uniformity.
- Treating Docker-dependent end-to-end tests as replaceable by mocks; both layers have
  different responsibilities.

## Acceptance Criteria

- [ ] T-0044 through T-0053 are Done with their task-scoped verification recorded.
- [ ] Every removed or weakened assertion has an explicit rationale and any stable
  behavior it protected remains covered elsewhere.
- [ ] New regression tests include counterfactual or negative-control evidence where
  practical, especially at security and cross-layer boundaries.
- [ ] Default and Docker CI suites have clearly separated, truthful responsibilities.
- [ ] Full format, lint, test, and build gates pass; live Docker checks pass in their
  supported environment.

## Workflow Route Rationale

- Cataloged route and risk: Initiative / High.
- Why this route: The work spans configuration, Feature acquisition, Docker invocation,
  lifecycle integration, forwarding, and public compatibility contracts.
- Why this risk gate: Two children touch secret exposure and untrusted OCI content;
  several others correct behavior that current tests misleadingly endorse.
- Escalation trigger: Split or record a decision when a child discovers a new product
  contract or a broader architecture change.

## Done When

All child tasks are complete and the suite's remaining assertions can be explained as
protecting a stable behavior, safety invariant, or compatibility boundary.
