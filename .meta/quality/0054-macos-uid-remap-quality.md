# T-0054 Quality Record

- Date: 2026-08-24
- Change: Extend `updateRemoteUserUID` mapping to macOS hosts while preserving
  Linux behavior and an explicit Windows/unsupported-host no-op.
- Route: Initiative
- Risk: High
- Owner or reviewer: T-0054 isolated implementer

## Readiness

| Area | Ready? | Evidence Or Required Action |
| --- | --- | --- |
| Outcome and scope | Yes | The user correction and T-0054 brief require macOS mapping, unchanged Linux behavior, and a Windows no-op; `remoteUser` and user namespaces remain out of scope. |
| Architecture | Yes | The existing build-stage remap already accepts host UID/GID values; the bounded change is platform eligibility plus host-ID discovery on macOS. |
| Safety | Yes | Existing root, numeric-user, disabled, unavailable-ID, in-image UID collision, and GID collision protections remain authoritative. |
| Verification | Yes | Establish a counterfactual platform test, run focused UID tests, then format, check, clippy, full tests, and build. Docker-free platform injection avoids requiring a macOS CI runner. |
| Rollback | Yes | Reverting the task commit restores the Linux-only platform gate without changing persisted data or runtime protocols. |

Verdict: **Ready**. The implementation does not change the remap script or image
ordering; it only allows the established safe remap plan to be selected on macOS.

## Acceptance And Verification

| Acceptance Criterion | Verification Method | Observed Evidence | Status |
| --- | --- | --- | --- |
| Simulated macOS plans an eligible named-user remap. | Focused `src/uid.rs` unit test with injected platform and IDs; run against pre-change behavior first. | The new test failed against the pre-change gate with `None { reason: NonLinuxHost }`, then passed with a `501:20` macOS remap plan after the change. | Pass |
| Linux retains its established mapping behavior. | Existing simulated-Linux unit test and full suite. | `simulated_linux_remaps_non_root_named_user` passed in the focused and 504-test unit runs. | Pass |
| Windows/unsupported hosts remain an explicit no-op. | Injected Windows/unsupported platform unit tests. | `simulated_windows_skips_remapping` and `simulated_unknown_platform_skips_remapping` passed with `UnsupportedHost`. | Pass |
| Disabled, root, numeric-user, and unavailable-ID safeguards remain. | Existing focused UID tests, exercised across supported-host planning. | Each planning safeguard now runs for both injected Linux and macOS; all focused tests passed. | Pass |
| Collision, already-matching, and GID-collision behavior remain in the generated remap block. | Existing remap-block tests and full suite. | The remap script is byte-unchanged; collision/already-matching assertions passed in focused and full tests. | Pass |
| Build/Dockerfile integration is unchanged except for platform eligibility. | Focused Feature-context/build tests and manual diff inspection. | All 17 `features::context::tests::dockerfile_` tests passed; diff review confirmed unchanged remap block, build args, and ordering. | Pass |
| Public and project documentation describe Linux/macOS mapping and Windows no-op. | Documentation search and diff review. | User guide, architecture, project context, decision 0002, and historical T-0026/T-0045 records were reconciled. | Pass |

## Required Checks

- Focused UID and generated-Dockerfile tests.
- `cargo fmt --check`.
- `cargo check`.
- `cargo clippy -- -D warnings`.
- `cargo test`.
- `cargo build`.
- `git diff --check` and scoped diff review.

Observed results:

- Focused UID tests: 15 passed.
- Focused generated-Dockerfile tests: 17 passed.
- `cargo fmt --check`: passed.
- `cargo check`: passed.
- `cargo clippy -- -D warnings`: passed with no warnings.
- `cargo test`: passed; 504 unit, 30 runnable CLI (3 ignored), 9 config,
  8 fake-Docker boundary, and 9 Feature CLI tests; 32 Docker smokes compiled
  and remained ignored under the project's local verification policy.
- `cargo build`: passed.
- `git diff --check`: passed.

## Review

An adversarial platform/safety pass found no blocking issue. The platform enum makes
macOS eligibility and the Windows no-op independently testable; `host_ids()` compiles
the `id` probes only on Linux/macOS; and the change does not touch the collision-safe
in-image script, Docker build argument construction, state hydration, or runtime
protocol. Reverting the commit remains the bounded mitigation.

## Residual Risk

This Linux environment cannot execute a real macOS Docker Desktop build, so the result
does not empirically prove Docker Desktop's runtime ownership behavior. The requested
platform decision and build inputs are covered deterministically without weakening the
existing CI-owned Docker smoke suite.
