# T-0044 Brief: Safe Feature Install Execution

## Identity And Source

- Task ID: T-0044
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

Running a devcontainer Feature does not unconditionally enable shell xtrace or expose
expanded secrets in build output, while Feature install scripts still execute with the
correct interpreter and content.

## Background

`src/features/context.rs` injects `PS4` and `set -x` into every `install.sh`, and a group
of tests asserts the precise injected text and placement. Those tests preserve a debug
implementation rather than a safe product property and conflict with the project rule
that secrets must not be logged.

## Scope

In scope:

- Remove unconditional tracing, or make any retained diagnostic mode explicit,
  off-by-default, and demonstrably safe.
- Replace `inject_trace_*` and generated-tar trace assertions with install-execution,
  shebang-preservation, byte/content, and no-secret-output behavior tests.
- Update user or maintainer documentation if a supported debug control changes.

Out of scope:

- Redesigning the whole Feature build pipeline.
- Claiming arbitrary third-party Feature scripts cannot print their own secrets.

## Acceptance Criteria

- [ ] Default generated Feature install scripts contain no forced `set -x` behavior.
- [ ] A representative secret expanded by an install command is not emitted by `dcc`'s
  own tracing behavior.
- [ ] Scripts with POSIX and Bash shebangs, and scripts without a shebang, retain valid
  execution behavior.
- [ ] Tests assert the safe execution contract rather than exact injected debug text.
- [ ] The focused test demonstrates failure against the pre-change tracing behavior or
  uses an equivalent negative control.

## Verification Plan

- Automated checks: focused Feature context tests, generated archive inspection, full
  `cargo test`, lint, format, and build.
- Security check: inspect output paths for expanded fixture secrets and run the project
  security checklist items concerning secret logging.
- Manual check: review the generated `install.sh` boundary and any debug documentation.

## Done When

Feature installation remains functional without default secret-revealing trace output,
and tests protect that outcome.
