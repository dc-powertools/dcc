# T-0007 Brief: Feature Metadata Compatibility Slice

## Goal

Implement the third T-0004 slice: bring Feature metadata handling in line with the
schema-compatible `customizations.dcc` model, project state mounts, and explicit unsafe
runtime gating.

## Scope

- Parse Feature `customizations.dcc.commands` and `customizations.dcc.state`.
- Preserve legacy Feature top-level `scripts` temporarily with warnings, using the same
  command resolution semantics already used for feature commands.
- Validate Feature state entries with the T-0006 state model and merge them before
  project config state for runtime mount planning.
- Preserve lifecycle hook collection order: Feature hooks in canonical feature
  installation order before project hooks.
- Parse Feature unsupported runtime properties `init` and `entrypoint`; warn that `dcc`
  owns PID 1/controller startup and ignores them.
- Treat Feature-provided `containerUser` and `remoteUser` as errors.
- Parse Feature unsafe runtime properties `privileged`, `capAdd`, and `securityOpt`;
  reject them by default and require `--allow-unsafe-runtime` for build and runtime use.
- Keep Feature metadata serialized through `devcontainer.metadata` so runtime-only
  behavior still works from the built image label.

## Non-Goals

- No devcontainer top-level `runArgs` support or safe allowlist; T-0010 owns that.
- No generated controller/hook assets or build-preparation execution; T-0008 owns that.
- No durable container lifecycle commands; T-0009 owns that.
- No cloud/container deployment mount restrictions beyond the unsafe Feature properties
  in this slice.

## Acceptance

- Feature `customizations.dcc.commands` and legacy `scripts` contribute to `dcc run`
  command listing/resolution with deterministic feature prefixes.
- Feature state entries are validated, serialized, parsed from image metadata, and
  planned as profile-local state mounts before project state.
- Feature `containerUser`/`remoteUser` fail clearly.
- Feature `init`/`entrypoint` produce build-time warnings and no runtime behavior.
- Feature unsafe settings fail clearly without `--allow-unsafe-runtime` and are included
  in runtime Docker args only when the flag is present.
- Focused Feature/runtime tests pass, plus required project checks for the commit.

## Verification Plan

- Unit tests in `src/features/`, `src/exec.rs`, and CLI flag tests for new option
  plumbing.
- Regression tests for Feature state validation, command metadata, unsupported warnings,
  and unsafe-property rejection/allowance.
- Required checks before commit: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, and `cargo build`.
