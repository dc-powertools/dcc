# T-0050 Brief: CLI-To-Docker Contract Tests

## Identity And Source

- Task ID: T-0050
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

Tests prove that user-facing compatibility and build/runtime options survive caller
logic and reach Docker with the correct arguments, rather than testing only isolated
argument helpers.

## Background

Current version tests cover semantic-version helpers but not real image-label inspection
and refusal paths. Docker build tests prove `--pull` appears when an internal boolean is
already set, but do not prove `--no-cache` sets that boolean correctly for image and
official-build profiles while excluding the generated local intermediate. Similar
cross-layer evidence is missing for memory and CPU limits.

## Scope

In scope:

- Introduce or reuse an argv-recording fake Docker executable/process seam.
- Exercise missing, incompatible, and patch-compatible `dcc.version` labels through
  runtime command entry points, including the rebuild instruction and best-effort stop
  semantics.
- Exercise `--no-cache` through both image and official `build` source paths, proving
  where `--pull` is and is not forwarded.
- Exercise supported `--memory` and `--cpus` inputs through the final run/create argv.
- Keep helper unit tests only where they add distinct edge-case value.

Out of scope:

- Retesting Docker's own argument parser.
- Adding flags not already part of `dcc`'s public contract.

## Acceptance Criteria

- [ ] A fake Docker boundary records the actual calls made by top-level command paths.
- [ ] Missing and major/minor-incompatible labels refuse runtime work with a rebuild
  command; patch drift proceeds.
- [ ] `--no-cache` causes upstream base pulls for both supported profile shapes and does
  not pull the locally generated intermediate tag.
- [ ] Memory and CPU limits reach the appropriate Docker invocation unchanged and are
  absent when unspecified.
- [ ] Mutating a caller to drop each option makes the corresponding test fail even if
  low-level helper tests still pass.

## Verification Plan

- Automated checks: focused fake-Docker integration tests, CLI tests, full tests, lint,
  format, and build.
- Manual check: inspect recorded argv for ordering around image names and entrypoint
  arguments, the class of regression previously found in CI.

## Done When

Caller-plumbing regressions at the CLI-to-Docker boundary are caught without requiring
live Docker for every case.
