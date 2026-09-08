# T-0085 Profile Running Indicator Notes

## Task State

- Revision: r3
- Status: Done
- Approved checkpoint: `cd0af75`
- Design: `.meta/tasks/0084-profile-running-indicator-design.md` r2
- Final implementation: Batched label-based status lookup, `[running]` text, tri-state
  JSON, runtime/build-prep roles with legacy fallback, truthful unavailable/unknown
  handling, reserved internal labels, and dry-run/empty zero-query behavior are complete.

## Implementation Correction

- Discovery: Runtime `runArgs` already permit Docker `--label`, and those arguments are
  appended after dcc-owned labels. A profile could therefore override
  `dcc.container_id` or the new `dcc.container_role` and make lookup/status untrustworthy.
- Required correction: Reject the two exact reserved label keys in split and
  `--label=...` forms, with focused tests. Other user labels remain supported.
- Revision impact: T-0085 advanced to r2 because reliable status requires preserving
  the internal identity/role labels at container creation.
- Dry-run correction: Global `--dry-run` promises no Docker invocation. Profile listing
  must therefore skip the status snapshot under dry-run, keep non-empty profile states
  unknown, and explain the skipped query without weakening the empty-list behavior.
- Revision impact: T-0085 advanced to r3 to preserve the established global dry-run
  boundary while adding automatic status lookup.

## Agent Roster

| Assignment | Ownership | Expected output | Restart policy |
| --- | --- | --- | --- |
| Docker query and roles | `src/docker.rs`, `src/build.rs`, `src/exec.rs`; inline tests in those files | Batched running-container query/parser plus runtime/build-prep labels | Reassign only if no edits or handoff are recoverable from these files. |
| Profile output and dispatch | `src/profile.rs`, `src/main.rs`; inline tests in `src/profile.rs` | Async tri-state enrichment, `[running]` renderer, warnings/debug, awaited dispatch | Reassign only if no edits or handoff are recoverable from these files. |
| CLI verification and docs | `tests/cli_flags.rs`, `tests/docker_boundary.rs`, `tests/docker_smoke.rs`, `README.md`, `docs/index.md`, `.meta/project/architecture.md` | Fake/live Docker coverage and aligned public/architecture docs | Reassign only if no edits or handoff are recoverable from these files. |

Workers do not edit `.meta/README.md`, `.meta/tasks/README.md`, this note, or files
outside their ownership. The Root Orchestrator owns integration, task records, full
verification, review, and commits.

## Verification Contract

- Focused profile, Docker-boundary, and CLI integration tests after integration.
- `cargo fmt --check`
- `cargo check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build`
- `git diff --check` plus focused correctness/compatibility review.
- Docker-capable CI owns the ignored live lifecycle smoke if the local daemon is not
  usable.

## Final Evidence

- `cargo fmt --check`: passed.
- `cargo check`: passed for `dcc v0.1.6`.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test`: passed with 568 unit and 87 runnable integration tests; 38 tests were
  ignored as designed.
- `cargo build`: passed.
- Focused `cargo test profile_list`: passed with four CLI and seven fake-Docker tests;
  the live lifecycle smoke compiled and was listed as ignored.
- `cargo run --quiet -- profile list --help`: passed and described running-environment
  markers.
- `git diff --check`: passed.
- Independent focused review: no remaining findings after strengthening both batched
  status tests to reject any extra Docker call.
- Live Docker smoke: not run because `docker` is absent from this environment. The
  fake-Docker suite covers exact argv, roles, true/false/null, failures, malformed
  output, dry-run, empty discovery, and one-query behavior; Docker-capable CI can run
  the ignored start/list/stop smoke.
