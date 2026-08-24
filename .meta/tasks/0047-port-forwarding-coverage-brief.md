# T-0047 Brief: Port Forwarding Behavioral Coverage

## Identity And Source

- Task ID: T-0047
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

The headline port-forwarding workflow has deterministic tests for successful data
relay, error reporting, and cleanup, and its behavior remains usable on hosts where one
IP family is unavailable.

## Background

`src/forward.rs` binds IPv4 and IPv6 listeners, spawns relay tasks, invokes `docker exec
... nc`, and copies traffic in both directions. Its test module is empty and says bind
errors are exercised indirectly, but no test directly covers the workflow. Requiring
both IP families and ending a relay when the first copy direction finishes are also
unverified behavior choices with user-visible consequences.

## Scope

In scope:

- Introduce narrow seams for connector/listener/process behavior without replacing the
  public end-to-end contract with mocks.
- Test bind conflicts, listener startup, bidirectional bytes, half-close/response drain,
  task cleanup, connector failure, and unavailable IPv6 behavior.
- Add one live Docker smoke that reaches a service in the container through the host
  forwarded port.
- Document any intentionally degraded single-stack behavior.

Out of scope:

- Supporting remote Docker daemons or UDP forwarding.
- General networking framework refactors unrelated to these contracts.

## Acceptance Criteria

- [ ] A local deterministic test proves bytes flow in both directions.
- [ ] Client half-close does not truncate a valid server response under the chosen
  contract.
- [ ] A bind collision reports the affected address/port and leaves no orphan task.
- [ ] Lack of IPv6 does not unnecessarily disable valid IPv4 forwarding, or an explicit
  contrary product decision is recorded.
- [ ] A live Docker smoke covers the actual `docker exec`/`nc` boundary.
- [ ] Negative controls or mutations show the tests fail when one relay direction,
  cleanup, or bind error handling is broken.

## Workflow Route Rationale

- Cataloged route and risk: Initiative / High.
- Why this route: The work combines asynchronous lifecycle design, local deterministic
  coverage, and a live container boundary.
- Why this risk gate: Detached task leaks and silent truncation affect a primary user
  workflow and can be timing-sensitive.

## Verification Plan

- Automated checks: focused async tests with bounded timeouts, full tests, lint, format,
  build, and serialized live Docker smoke.
- Flake check: repeat timing-sensitive focused tests without rerunning failures until
  green; investigate any inconsistent result.

## Done When

The forwarding tests fail for meaningful relay, lifecycle, and availability regressions
and do not depend on arbitrary sleeps.
