# Development Guide

This guide is for people maintaining `dcc`.

## Project Shape

`dcc` is a Rust 2021 single-binary CLI. Docker integration goes through Docker
CLI subprocesses, and user-facing behavior should stay documented in the public
README or [user guide](index.md).

Useful entry points:

- `src/cli.rs`: CLI definitions.
- `src/main.rs`: command dispatch.
- `src/config/`: devcontainer parsing, merge, and substitution.
- `src/build.rs`: image build and build-preparation hooks.
- `src/run.rs`, `src/exec.rs`, `src/stop.rs`: runtime workflows.
- `src/supervisor.rs`: generated in-container lifecycle supervisor scripts.
- `tests/`: CLI and integration coverage.

## Local Setup

Install the Rust formatting and lint components once:

```sh
rustup component add rustfmt clippy
```

Docker is required for real `dcc build`, `run`, `exec`, `attach`, and `stop`
workflows. The regular local test suite does not run Docker smoke tests by
default.

## Development Loop

Read the relevant source and tests before changing code. Work in small,
verifiable steps and prefer existing project types, helpers, and module
boundaries over new abstractions.

Fast checks while working:

```sh
cargo check
cargo test
```

Before finishing a change, run:

```sh
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

For behavior that affects the compiled binary or release artifact, also run:

```sh
cargo build
```

Docker-dependent smoke tests are ignored locally and run explicitly in CI on
GitHub-hosted Ubuntu runners with Docker available.

## Documentation Changes

Keep documentation current with behavior:

- End-user overview and quick start: `README.md`.
- Detailed user-facing behavior and configuration: `docs/index.md`.
- devcontainer Feature package behavior: `docs/features.md`.
- Local development and release workflow: `docs/development.md`.
- Stable internal architecture notes: `.meta/project/architecture.md`.
- Canonical project command catalog: `.meta/project/standards.md`.

Avoid duplicating long reference material between documents. Link to the owning
document instead.

## Scope And Quality

Keep patches logically scoped to the requested behavior. Do not fold unrelated
refactors into feature or bug-fix work.

Before committing, read the diff critically:

- The change satisfies the request.
- Every changed line belongs to the task.
- Errors that reach users are diagnosable.
- Tests exercise observable behavior.
- Public behavior changes are documented.

## Releasing

To cut a release, bump the version and push to `main`:

```sh
scripts/bump.sh patch     # or: minor | major
git push origin main
```

`scripts/bump.sh` edits the version in `Cargo.toml`, refreshes `Cargo.lock`, and
commits `chore: bump version to vX.Y.Z`.

When the push lands on `main`, the **Auto-tag on version change** workflow
(`.github/workflows/autotag.yml`) runs CI. If CI passes, it creates the matching
`vX.Y.Z` tag when needed, which triggers the **Release** workflow to build the
target binaries and publish a GitHub Release. If CI fails, no tag or release is
produced.

You can also run the **Bump Version** workflow from the Actions tab and choose
`patch`, `minor`, or `major`; it performs the same version bump in CI.

Do not push or publish a release unless the project owner explicitly asks for
that action.
