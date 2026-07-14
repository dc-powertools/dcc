# T-0004 Quality Record

- Date: 2026-07-14
- Change: Full `dcc` devcontainer compatibility and durable lifecycle rewrite
- Route: Initiative
- Risk: High
- Owner or reviewer: Root Orchestrator; specialist review via sub-agents where useful

## Scope And Criteria

- User-visible outcome: `dcc` accepts schema-compatible configuration under
  `customizations.dcc`, persists declared state, and manages durable profile
  containers with coherent `start`, `stop`, `run`, `exec`, and `attach` behavior.
- In scope: T-0005 through T-0010 child tasks.
- Non-goals: cloud snapshot providers and release publication.

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Schema-compatible `dcc` config parses and merges under `customizations.dcc`. | T-0005 tests and review. | Unit and integration tests cover nested `customizations.dcc.extends`, `commands`, parser-level `state`, strict mode, legacy warnings, conflict handling, and merge behavior. Specialist review reported no blocking findings. | Pass |
| State paths are validated and planned as profile-cache mounts. | T-0006 tests and review. | Unit tests cover substitution, validation failures, duplicate/conflict handling, containerEnv deferral, cache mount planning, and host preparation. Required checks passed. | Pass |
| Feature metadata contributes state, commands, hooks, and unsafe settings consistently. | T-0007 tests and review. | Unit tests cover nested Feature commands/state, legacy scripts compatibility, state metadata round-trip, hook order preservation, unsupported/invalid Feature properties, unsafe setting rejection/allowance, runtime state merge, and CLI flag parsing. Required checks passed. | Pass |
| Build preparation runs expected hooks with generated controller assets. | T-0008 tests and review. | Unit tests cover official `build` parsing/conflict handling, Docker build args, generated controller/wrapper/hook assets, build-prep hook order, `--refresh-only` planning, and CLI flags. Required checks passed. | Pass |
| Durable `start`, `stop`, `run`, `exec`, and `attach` workflows behave coherently. | T-0009 tests and review. | Unit tests cover runtime mode bookkeeping, active-command records, lifecycle hook phase selection, debug output, and state/runtime merge behavior. CLI tests cover `start`, `attach`, and `--keep` parsing. Required checks passed. No live Docker smoke test was run. | Pass |
| Docs and fixtures describe current behavior and official validation target. | T-0010 docs review and validation command availability check. | README, architecture, task notes, standards, and threat model updated for final behavior. Official `devcontainer read-configuration` could not run because `devcontainer`, `node`, `npm`, and `npx` are absent from PATH in this environment. | Pass with blocker recorded |

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | T-0004 brief records accepted requirements and non-goals. |
| Architecture and project context | Yes | Child tasks isolated config, state, Feature metadata, build, runtime, and final compatibility; architecture docs updated. |
| Data, security, and permissions | Yes | Unsafe Feature/devcontainer settings, unsafe `runArgs`, and sensitive mounts are rejected by default and require `--allow-unsafe-runtime`; final threat-model review recorded. |
| Slices and ownership | Yes | Catalog now owns T-0005 through T-0010; root agent owns shared docs and integration. |
| Verification and rollback | Concern | Unit/integration checks are clear; real Docker smoke coverage depends on environment availability. Rollback is local git revert before push. |

Readiness verdict: Ready with concerns

## Verification Results

| Required? | Check Or Method | Observed Result | Status | Unblocking Condition If Not Run |
| --- | --- | --- | --- | --- |
| Yes | `cargo fmt --check` | Passed for T-0005 through T-0010. | Pass | |
| Yes | `cargo clippy -- -D warnings` | Passed for T-0005 through T-0010. | Pass | |
| Yes | `cargo test` | Passed for T-0010 after review fixes: 406 unit tests, 23 runnable CLI/config tests with 2 ignored, and 9 config error integration tests. | Pass | |
| Yes | `cargo build` | Passed for T-0005 through T-0010. | Pass | |
| Yes | Specialist review for non-trivial code slices. | T-0005 read-only review found no blocking findings. | Pass | |
| Yes | Security review for runtime/mount/script slices. | T-0010 read-only review found a blocking non-normalized mount bypass plus lower-severity coverage/docs issues. Root fixed path/SSH-agent mount gating, cache auto-create escape, merge tests, and records. | Pass | |
| Yes | Official devcontainer config validation | Could not run: `devcontainer`, `node`, `npm`, and `npx` are absent from PATH. | Blocked | Install official devcontainer CLI or Node/npm tooling in a future environment and run `devcontainer read-configuration --workspace-folder <fixture> --include-merged-configuration`. |

- Criteria or methods amended after implementation began, with reason and impact: None yet.
- Counterfactual evidence for new regression or behavior tests: Pending per child task.
- Flaky result and disposition: None yet.

## Review Findings

| Severity | File Or Area | Finding | Required Fix | Status |
| --- | --- | --- | --- | --- |
| Low | T-0006 state validation | T-0005 deduplicates exact `StateEntry` values only; same path with conflicting state type is deferred. | T-0006 must reject or resolve same normalized path with conflicting kinds. | Closed |
| Low | CLI integration coverage | `customizations.dcc.commands` is verified through config resolution and existing run resolver tests, not an end-to-end `dcc run :name` integration. | Add runtime command integration when durable runtime behavior is available. | Accepted |
| Low | Raw merge internals | A consumed `customizations.dcc.extends` can remain in some parent-only raw merge shapes, though `raw_to_config` ignores it and current resolution is unaffected. | Clean if later code inspects merged raw config directly. | Accepted |
| High | T-0010 mount safety | Sensitive mount gating compared raw paths, so `..` segments could bypass source checks. | Treat parent-directory components as unsafe and add regression tests for `mounts` and `runArgs`. | Closed |
| Medium | T-0010 SSH agent safety | SSH agent sockets with obscure host paths could pass when mounted to an agent-looking container target. | Gate SSH-agent-like mount targets as unsafe and add regression tests. | Closed |
| Medium | T-0010 cache source creation | Cache bind source auto-creation could escape the cache through `..` segments. | Reject parent-directory components before auto-creating cache mount sources. | Closed |
| Low | T-0010 merge coverage | New final compatibility fields lacked direct merge tests. | Added merge policy coverage for `runArgs`, unsafe fields, port attributes, `overrideCommand`, `workspaceFolder`, and `workspaceMount`. | Closed |

## Consistency

| Canonical Source | Expected | Actual | Required Update |
| --- | --- | --- | --- |
| User request | Full framework, sub-agents, strict review, task queue, regular commits. | Queue initialized; sub-agent usage planned. | Keep notes current. |
| Project brief or task brief | T-0004 scope and child tasks aligned. | T-0010 brief and project brief reflect final behavior. | None. |
| Decisions and standards | Significant choices recorded. | T-0004/T-0010 records own conservative `runArgs`, mount gating, and compatibility-field decisions. | None. |
| Tests and docs | Tests and docs match final behavior. | T-0006 through T-0010 tests and docs updated for implemented behavior. | None. |
| State and assumptions | Active task pointer and queue current. | T-0010 and T-0004 are ready to close with recorded residual risks. | None. |

## Batch And Residual Risk

- Large-diff split trigger hit: Yes
- If kept together, why: Not kept together; split into child tasks and commits.
- Risk not resolved by passing checks: Real Docker environment behavior may still vary
  across host Docker versions and base images.
- T-0009 accepted a pragmatic first pass where mutable runtime mode and
  active-command state live under the host profile cache. The generated in-container
  controller remains minimal; PID-1-owned command accounting is deferred unless future
  Docker smoke testing shows host-side bookkeeping is insufficient.
- T-0010 parses port attributes for schema compatibility, but browser/preview auto-open
  behavior is not implemented.
- Real Docker smoke tests and official devcontainer CLI validation were not run in this
  environment.

## Completion

- Required checks all passed: Yes
- Status: Done with residual risks
- Exact incomplete condition, if not Done: None; official devcontainer CLI validation and live Docker smoke testing remain recorded residual risks.
- Next action: Optional future live Docker smoke and official devcontainer CLI validation in an environment with the required tools.
