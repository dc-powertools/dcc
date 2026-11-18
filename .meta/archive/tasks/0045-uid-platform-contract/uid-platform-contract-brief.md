# T-0045 Brief: Linux-Only UID Remapping Contract

> Superseded platform outcome: T-0054 implements the product owner's later
> clarification that macOS participates in UID/GID mapping. This brief remains the
> historical record of the Linux-only correction completed by T-0045; its injectable
> platform-decision design and explicit Windows no-op remain current.

## Identity And Source

- Task ID: T-0045
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

`updateRemoteUserUID` remaps users only for Linux hosts, and the test suite proves the
Linux and non-Linux decisions independently of the CI host platform.

## Background

The public contract and devcontainer reference describe Linux-only remapping, but
`src/uid.rs` currently treats every Unix host as eligible. The `#[cfg(unix)]` test
therefore endorses remapping on macOS, while `host_ids_does_not_panic` asserts no useful
result.

## Scope

In scope:

- Separate host platform detection from remap planning enough to exercise Linux,
  macOS/non-Linux, disabled, root, numeric-user, match, and collision branches in one
  deterministic suite.
- Correct the implementation to skip remapping outside Linux.
- Replace the no-panic smoke with assertions about returned host IDs or explicit
  unavailable behavior.
- Reconcile T-0026 documentation if it still describes the broader Unix gate.

Out of scope:

- Implementing `remoteUser` or Podman user namespaces.
- Depending on a macOS runner merely to test a pure platform decision.

## Acceptance Criteria

- [ ] A simulated Linux host plans valid non-root remapping.
- [ ] Simulated macOS and Windows/non-Linux hosts produce an explicit no-op.
- [ ] Collision, already-matching, root, numeric-user, disabled, and unavailable-host-ID
  safety branches remain covered.
- [ ] The old Unix-wide behavior is shown to fail the non-Linux regression test.
- [ ] Public and project documentation consistently say Linux-only.

## Verification Plan

- Automated checks: focused UID tests on the local Linux host, full tests, lint, format,
  and build.
- Manual checks: compare the abstraction and documented outcome with the accepted
  T-0026 Linux-only decision.

## Done When

Host-independent tests would catch any future reintroduction of UID remapping on macOS
or other non-Linux platforms.
