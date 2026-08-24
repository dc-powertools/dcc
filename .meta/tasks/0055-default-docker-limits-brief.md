# T-0055 Brief: Default Docker Resource Limits

## Identity And Source

- Task ID: T-0055
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User request
- Source reference and date: Product-behavior correction, 2026-08-24
- Related task: T-0050

## Goal

Containers created by `dcc` receive implicit Docker limits of 4 GiB memory and 2 CPUs
unless the user supplies explicit resource values.

## Background

T-0050 changed unspecified memory and CPU options to omit Docker resource flags and
removed documentation of the earlier defaults. The intended design is the opposite:
the 4g/2cpu limits are product defaults, while explicit CLI values remain overrides.

## Scope

In scope:

- Restore the effective default memory limit to `4g` and CPU limit to `2` for Docker
  container creation paths governed by these runtime options.
- Preserve explicit `--memory` and `--cpus` values unchanged.
- Ensure the flags appear in valid Docker argv position before the image.
- Update public help/documentation and project architecture wherever the default was
  removed or described as optional.
- Reverse the T-0050 unspecified-value test and retain cross-layer fake-Docker coverage.

Out of scope:

- Adding dynamic host-size-based resource selection.
- Applying limits to Docker build operations unless they already share this documented
  runtime contract.

## Acceptance Criteria

- [ ] Omitting both options sends `--memory 4g` and `--cpus 2` to every applicable
  Docker container-creation invocation.
- [ ] Supplying either option overrides only that resource and retains the other
  product default.
- [ ] Recorded argv proves both values precede the image argument.
- [ ] CLI help and user/project documentation state the implicit defaults.
- [ ] A regression that omits the default flags fails the Docker-boundary tests.

## Verification Plan

- Automated checks: focused fake-Docker boundary and CLI tests, help snapshots/searches,
  full tests, lint, format, and build.
- Manual check: inspect all container creation paths for consistent default application.

## Done When

Users receive the intended 4g/2cpu limits without supplying flags, explicit overrides
still work, and the behavior is documented and enforced at the Docker boundary.
