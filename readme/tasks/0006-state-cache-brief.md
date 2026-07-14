# T-0006 Brief: State Path Validation And Cache Mount Planning

## Goal

Implement the second T-0004 slice: validate `customizations.dcc.state` entries and plan
profile-local host cache mounts for each accepted state path.

## Scope

- Validate resolved state entries from project config after merge.
- Preserve the T-0005 state model: string entries are directories; object entries may
  declare `{ "path": "...", "type": "file" }`.
- Accept absolute container paths, including supported container-side variable forms
  once resolved.
- Reject unresolved values, relative paths, `..`, `/`, duplicate normalized paths with
  conflicting kinds, overlapping parent/child paths, runtime/system paths, and reserved
  `/workspace/.dcc` state.
- Add a deterministic profile-local cache mount plan for each state entry.
- Create host directories or parent directories needed for planned bind mounts before
  container startup.
- Include state mounts in debug/runtime mount planning without replacing the existing
  `/cache` mount yet.

## Non-Goals

- No Feature metadata state contributions; T-0007 owns Feature state.
- No generated controller or lifecycle-preparation behavior; T-0008 owns that.
- No durable `start`/`stop`/`attach` lifecycle behavior; T-0009 owns that.
- No official Dev Container CLI fixture validation unless it is cheap and already
  available.

## Acceptance

- Invalid state declarations fail during config loading with clear path-specific errors.
- Valid state declarations produce deterministic bind-mount strings rooted under the
  profile cache.
- Directory and file state use distinct host preparation behavior.
- Exact duplicate compatible entries deduplicate; conflicting kinds and overlapping
  paths error.
- Existing mounts, command resolution, cache behavior, and strict mode remain green.

## Verification Plan

- Unit tests for state normalization, validation failures, duplicate/conflict handling,
  cache mount planning, and host path preparation.
- Existing config/CLI tests remain green.
- Required checks before commit: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, and `cargo build`.
