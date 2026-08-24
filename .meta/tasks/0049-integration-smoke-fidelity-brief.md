# T-0049 Brief: Integration And Docker Smoke Fidelity

## Identity And Source

- Task ID: T-0049
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

CLI integration and Docker smoke tests directly observe the success, identity,
lifecycle, reuse, and teardown properties named by each test, and Docker-free tests run
without a Docker binary.

## Background

Several current tests pass when the command fails for an unrelated reason or when a
fresh container is created instead of reused. Start/stop and one-shot tests often assert
only command success, sleep, or launch a second command without querying the container
state. The nominally Docker-free seeding dry-run lives in `docker_smoke` and its fixture
destructor panics if `docker` is absent.

## Scope

In scope:

- Strengthen `tests/config_errors.rs` path-profile cases to require intended success and
  compare actual IDs across commands.
- Strengthen lifecycle smokes to query container IDs, labels, timestamps, running state,
  or an observable hook counter as appropriate.
- Assert one-shot teardown and durable reuse directly, without arbitrary sleeps as the
  acceptance mechanism.
- Remove assertions about retired host bookkeeping directories unless absence is a
  current security or public contract.
- Move Docker-free dry-run coverage to the default suite or make fixture cleanup safely
  tolerate an absent Docker executable.

Out of scope:

- Eliminating live Docker tests in favor of mocked commands.
- Combining unrelated lifecycle scenarios into one long smoke merely to reduce setup.

## Acceptance Criteria

- [ ] Path-profile config tests assert the expected exit status/output and compare the
  same container ID across applicable commands.
- [ ] Start-then-stop tests prove no matching container remains running.
- [ ] One-shot drain tests identify the original container and prove it exits/removes.
- [ ] Durable reuse and `--keep` promotion tests prove identity continuity or an
  equivalent persistent side effect.
- [ ] Docker-free dry-run tests pass when `docker` is absent, including fixture cleanup.
- [ ] Test names describe exactly the state observed; no test passes on an unrelated
  fatal error.
- [ ] New assertions have negative controls showing a fresh-container or no-teardown
  implementation would fail.

## Verification Plan

- Automated checks: default `cargo test`; focused Docker smokes serialized in a
  Docker-capable environment; lint, format, and build.
- Flake check: replace fixed sleeps with bounded polling/state queries and record any
  timing residual risk.
- Manual check: compare every edited test name with its final assertion.

## Done When

Each integration/smoke test can be explained by the user-visible state it observes, and
the Docker-free subset truly has no Docker dependency.
