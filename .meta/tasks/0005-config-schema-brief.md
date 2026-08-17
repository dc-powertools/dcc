# T-0005 Brief: Config Schema Compatibility Slice

## Goal

Implement the first T-0004 slice: move `dcc`-specific project config into
`customizations.dcc` while preserving temporary compatibility for legacy top-level
`extends` and `scripts`.

## Scope

- Parse `customizations.dcc.extends`, `customizations.dcc.commands`, and
  `customizations.dcc.state`.
- Keep legacy top-level `extends` and `scripts` for transition, emitting deprecation
  warnings in normal and strict modes.
- Use `customizations.dcc.extends` for merge-chain resolution; reject conflicting
  top-level and nested extends values in one file.
- Merge `customizations.dcc.commands` with existing script resolution semantics:
  feature commands remain `<feature-id>:<command>`, devcontainer commands remain
  `:<command>`, and unqualified names are accepted only when unique.
- Add a structured state-entry model, but only parser and merge behavior in this slice.
  Path validation and cache behavior belong to T-0006.
- Continue to parse official top-level devcontainer fields already supported by `dcc`.

## Non-Goals

- No state cache mounting behavior.
- No Feature metadata migration.
- No durable container lifecycle changes.
- No official Dev Container CLI fixture validation unless cheap and already available.

## Acceptance

- Configs using `customizations.dcc.extends` load and merge in parent-to-child order.
- Configs using `customizations.dcc.commands` populate the existing command map used by
  `dcc run`.
- Configs using `customizations.dcc.state` parse, merge, and deduplicate enough for
  later validation.
- Legacy top-level `extends` and `scripts` still work with deprecation warnings.
- Strict mode accepts `customizations` and does not reject recognized nested `dcc`
  keys.
- Focused parser/merge tests pass, plus required project checks for the commit.

## Verification Plan

- Unit tests in `src/config/` for parsing, merge, conflict, strict-mode, and legacy
  compatibility behavior.
- Existing CLI/config integration tests remain green.
- Required checks before commit: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, and `cargo build`.
