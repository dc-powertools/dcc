# T-0113 execution r9

## Authority and frame

User requested execution/orchestration with agents on 2026-11-18. This supersedes
r8's design-only limitation for its selected startup/admission slice. Broader task
completion, post-exit Docker result delivery and pre-command disconnect detection
remain deferred; no journal, CLI acknowledgement, replay or grace delay is selected.
Route: Initiative; risk: High (lifecycle concurrency and process signalling).
Readiness: ready with bounded concerns about cross-user lock access, flock packaging,
and baked-script compatibility; assess and verify before delivery.

## Plan and acceptance

1. Implement one exclusive lifecycle flock boundary for registration, retirement,
   stopping, promotion and orphan expiry; keep waits, hooks and user children outside.
2. Persist complete PID/start-time records; supervisor retires only proven dead,
   changed-identity or terminated wrappers; preserve/diagnose unknown records.
3. Qualify packaging, root/non-root controls and compatibility; minor bump if the
   registration/control protocol changes, with mismatch gate regressions.
4. Deterministically verify both admission/drain lock orders, last-record retirement,
   between-poll completion, process identity states, SIGKILL lock release, descriptor
   closure, disconnected live/finite commands and unchanged mode/stop semantics.
5. Run fmt, locked check, Clippy, full locked tests/build and relevant live Docker
   regressions; focused independent review and security review. Record counterfactuals.
6. Update current docs and task evidence; create a verified local commit, no push.

Docker is reachable with sudo (Engine 29.9.3); use exact temporary resources and
existing tests. No host/daemon permissions or production resources are changed.
Rollback: revert local change and rebuild images; no durable state schema changes.

## Agent ownership

- Root: orchestration, task/current documentation, version compatibility, integration,
  verification and local commit. Sole writer of shared project/task records.
- Implementer: src/supervisor.rs and src/features/context.rs only; lifecycle scripts,
  flock packaging and affected existing unit assertions. Reports tests and concerns.
- QA: new src/supervisor_lifecycle_tests.rs only; independent deterministic Linux local
  behavioral tests using generated scripts. Root wires module; no shared-doc edits.
- Reviewer (after implementation): read-only correctness/security review and proposed
  findings. No source or shared-doc writes.

Workers may resume only their assigned owned files, inspecting current state first;
root integrates outputs and resolves overlaps. Do not commit or push independently.

## Delivered implementation and compatibility

The selected slice is implemented in `src/supervisor.rs` and universally packaged
through `src/features/context.rs`. All three baked scripts share one generated
shell implementation. PID 1 alone creates the stable 0644 lock; participants open
it read-only and close fd 9 outside critical sections, supporting root controls
against a non-root-owned sticky tmpfs. A SIGTERM trap closes any existing lock
before committing shutdown, avoiding self-deadlock.

The wrapper captures its own kernel start time and publishes a complete record
with an atomic move from staging outside `active`. SIGKILL during staging cannot
leave an incomplete active record. It retains the record on all exits; PID 1
retires only proven dead/changed/terminated identities and remembers accepted work.
Unknown records block drain with bounded diagnostics. Stop-now snapshots identities
under the lock and rechecks before signalling outside it; this is not pidfd-atomic.

The registration/control contract changes, so 0.3.1 → 0.4.0 is required by the
patch-stability rule. Previous-image rejection and bidirectional minor-version
compatibility tests passed; patch compatibility remains allowed. Existing 0.3
containers need explicit stop, image rebuild and recreation. No release, push or
persistent-data migration was performed.

## Observed verification (2026-11-18)

| Check | Observed result |
| --- | --- |
| `cargo fmt --check` | Passed after formatting the new smoke tests. |
| `cargo check --locked` | Passed, dcc 0.4.0. |
| `cargo clippy --all-targets -- -D warnings` | Passed, no warnings. |
| `cargo test --locked --quiet` | Passed on final source: 592 unit + 125 runnable integration tests; 57 intentionally ignored by default. |
| `cargo build --locked` | Passed. |
| `cargo test --locked --bin dcc lifecycle_tests -- --nocapture` | All 18 passed; included in the full-suite total. |
| `cargo test --locked --test docker_smoke -- --ignored --nocapture --test-threads=1 --skip lifecycle_lock_root_controls_preserve_admission_boundary` | All 35 then-existing regressions passed; new disconnect smoke was added afterward and run separately below. |
| `cargo test --locked --test docker_smoke lifecycle_lock_root_controls_preserve_admission_boundary -- --ignored --exact --nocapture` | Passed across Debian bookworm-slim / Alpine 3.22 × dev / root, including Alpine flock installation, real lock creation and cross-user root mode/stop/stop-now. |
| `cargo test --locked --test docker_smoke lifecycle_disconnected_finite_job_retires_without_cli_acknowledgment -- --ignored --exact --nocapture` | Passed for dev/root: kill the actual dcc CLI after command admission, observe live wrapper, release finite job, verify completion marker and automatic one-shot removal. |
| Independent Docker-client SIGKILL probe | Passed: kill the exact Docker exec client (not its sudo parent); wrapper remains live/registered, finite work completes and PID 1 removes the one-shot without CLI acknowledgment. |
| Diff, documentation links and bounded cursor | Passed; no unrelated changes or framework policy edits. |

Live test commands used a temporary PATH wrapper executing
`sudo -n /usr/local/bin/docker "$@"`, without changing host or daemon permissions.
Engine 29.9.3 was observed. Fixture cleanup removes exact managed containers/images;
independent probe also asserted its exact temporary container was absent.

The probe method was `/tmp/dcc-0113-client-loss-qa.py`, with temporary results under
`/tmp/dcc-0113-client-loss-s9drdoef/results.json`. Durable observation: Docker client
exited -9, wrapper PID/start record stayed present/live, the finite job wrote its
finished marker and one-shot auto-removal completed within eight seconds. An initial
probe killed only sudo's process group and timed out because sudo isolated the
Docker child; that harness limitation was corrected by identifying/killing the exact
Docker descendant. It was not a product failure or a rerun-until-green result.

## Regression integrity

The Linux tests use generated scripts with temporary path rewrites and test-only
checkpoints to force registration-first and drain-first orders, including initial
empty observations, already-expired orphan deadlines and last-record retirement.
The fixture permits its pre-created lock inode only in the temporary supervisor;
production sole-creator initialization is exercised by the live smoke.

Coverage includes between-poll completion, persisted SIGKILL records, lock release
on SIGKILL, PID/start mismatch, synthetic unknown/read failures, malformed records,
actual zombies and stopped wrappers, kernel comm with spaces/parentheses/newline,
closed descriptors in user children/readiness/shutdown hooks, stop-now signal
snapshot rechecks, supervisor TERM while locked, and promotion before/after drain.
Existing smokes preserve one-shot/durable/keep, startup readiness and stop variants.

Counterfactuals, performed without changing the shared worktree:

- Extracting the HEAD-before-change wrapper/control scripts into a temporary tree,
  a fast command exiting 17 left zero records and an ARRIVED marker. The new
  retained-record/no-ARRIVED test would fail that original behavior.
- A current wrapper child verified fd 9 absent and returned 0. Mutating only the
  temporary wrapper to omit admission unlock made that same check return 1.

## Independent review and outcome

Implementer owned the two production source files; QA owned the new Linux test
module; root integrated versioning, smoke tests, current docs and task evidence.
Workers `implement_lifecycle`, `lifecycle_qa` and `lifecycle_review` all finished
their assignments and reported no unintegrated work.
Independent architect/security review found no blocking source or security issues,
confirmed protocol bump requirements, checked actual zombie and complex-comm
parsing, and reviewed test strength. Its one documentation precision finding was
fixed: status 253 is guaranteed at the wrapper admission boundary, not for every
Docker exec failure before wrapper admission. Its live disconnection evidence gap
was closed by the new smoke and the separate exact-client SIGKILL probe.

The change is kept in one commit because the lock/record implementation, baked
flock provisioning and minor-version fence must ship together. Most added lines
are deterministic regression tests and durable evidence; independent review covered
the production boundary and its tests as one coherent lifecycle repair.

All required checks for this bounded slice passed. Parent T-0113 remains Ready,
not Done: post-exit Docker status/output delivery is still unresolved and deliberately
deferred. No journal, CLI acknowledgment, automatic replay or extra delay was added.
Pre-command disconnection detection remains separately deferred. Desktop/macOS and
RPM distro behavior were not qualified by these Linux Debian/Alpine results; the
existing broader T-0097/T-0108 qualification gates remain open.

Next safe action: select the deferred result-delivery design/scope when that work
is resumed; do not silently implement the unselected journal. The current slice is
ready for a task-scoped local commit after final documentation/diff checks.
