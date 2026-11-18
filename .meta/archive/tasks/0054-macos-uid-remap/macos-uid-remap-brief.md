# T-0054 Brief: macOS `updateRemoteUserUID` Mapping

## Identity And Source

- Task ID: T-0054
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User request
- Source reference and date: Product-behavior correction, 2026-08-24
- Related tasks: T-0026, T-0045

## Goal

`updateRemoteUserUID` performs the intended user and group ID mapping on macOS hosts as
well as Linux hosts, without weakening existing collision and identity safeguards.

## Background

T-0045 deliberately restricted UID remapping to Linux and documented macOS as a no-op.
The product contract is now clarified: macOS must participate in mapping. The prior
Linux implementation and its safety cases provide the baseline, but macOS host-ID and
Docker ownership semantics must be represented deliberately rather than restored by a
broad Unix compile-time gate alone.

## Scope

In scope:

- Define and implement host UID/GID discovery and remap planning for macOS.
- Preserve the existing Linux mapping contract and an explicit Windows no-op.
- Keep disabled, root, numeric-user, already-matching, unavailable-ID, and in-image
  collision behavior safe and deterministic.
- Exercise Linux, macOS, and Windows decisions through injected platform tests that do
  not require every CI host OS.
- Reconcile public documentation, project architecture, and the T-0026/T-0045 design
  record with the corrected macOS contract.

Out of scope:

- Implementing top-level `remoteUser` support.
- Changing unrelated user-namespace or rootless-container behavior.

## Acceptance Criteria

- [ ] An enabled, named, non-root container user gets an appropriate remap plan on a
  simulated macOS host when usable host IDs are available.
- [ ] Linux retains its established mapping behavior and Windows remains an explicit
  no-op.
- [ ] Root, numeric-user, disabled, collision, already-matching, and unavailable-ID
  branches remain covered.
- [ ] Dockerfile/build/runtime behavior remains internally consistent with the planned
  UID/GID mapping.
- [ ] Public and project documentation describe macOS and Linux behavior accurately.

## Verification Plan

- Automated checks: focused host-independent UID tests, CLI/build-boundary coverage as
  needed, full tests, lint, format, and build.
- Manual check: inspect the resulting Dockerfile and ownership behavior for both Linux
  and macOS platform decisions.

## Done When

The suite proves that macOS and Linux both map eligible users according to the intended
contract, Windows does not, and users can find that behavior in the documentation.
