# T-0008 Brief: Build Preparation And Controller Assets

## Goal

Implement the fourth T-0004 slice: support official devcontainer `build` sources and
make `dcc build` perform deterministic preparation with state mounts and generated
controller/hook assets.

## Scope

- Parse official top-level `build` as an alternative to `image`; fail clearly if both are
  set.
- Support Dockerfile/context build inputs sufficiently for local `dcc build`.
- Keep existing image fast path when a profile uses only `image` and no `dcc` build-time
  changes.
- Generate initial `dcc` controller, command wrapper, and lifecycle hook assets into the
  image/build context.
- Materialize collected build-preparation hooks in deterministic order:
  `onCreateCommand`, `updateContentCommand`, then `postCreateCommand`; Feature hooks in
  canonical Feature order before project hooks for each phase.
- Make `dcc build` run preparation by default after the image is available, with state
  mounts attached.
- Add `dcc build --refresh-only`: skip image rebuild, skip `onCreateCommand`, run only
  `updateContentCommand` and `postCreateCommand`, and fail clearly if the profile image
  does not already exist.
- Ensure `--no-cache` and `--update` do not reset local state.

## Non-Goals

- No durable `start`, `stop`, `run`, `exec`, `attach`, one-shot bookkeeping, or `--keep`
  promotion; T-0009 owns runtime lifecycle commands.
- No full port attributes, safe `runArgs`, official validation fixtures, or final docs;
  T-0010 owns those.
- No cloud snapshot provider or state reset command.

## Acceptance

- Configs can use either `image` or official `build`, but not both.
- Build contexts include generated controller/wrapper/hook assets in deterministic
  locations.
- `dcc build` runs build-prep hooks with profile state mounts attached.
- `dcc build --refresh-only` skips rebuild and `onCreateCommand`, fails when no image
  exists, and runs update/post-create hooks when an image exists.
- Focused parser/build-planning tests pass, plus required project checks for the commit.

## Verification Plan

- Unit tests for `build` parsing, image/build conflict, build source planning, generated
  asset contents, hook materialization order, and `--refresh-only` planning.
- Existing CLI flag tests updated for `--refresh-only` if needed.
- Required checks before commit: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, and `cargo build`.
