# Project Context

Use this file as the concise implementation context that agents load before code
changes. It complements `.meta/project/brief.md`: the brief explains product intent,
while this file captures technical conventions that prevent inconsistent implementation.

## Scope

- Project/service: `dcc` Rust CLI.
- Last updated: 2026-07-14
- Applies to: Rust source, tests, CLI behavior, devcontainer config handling, Docker
  integration, and project documentation.
- Does not apply to: The reusable framework under `.meta/meta/`, except when explicitly
  maintaining the framework itself.

## Technology Stack

| Area | Choice | Version/Constraint | Source |
| --- | --- | --- | --- |
| Runtime | Rust binary | Edition 2021; observed Rust/Cargo 1.96.0 locally | `Cargo.toml`; command verification |
| CLI | `clap` derive | Dependency version `4` | `Cargo.toml`; `src/cli.rs` |
| Async/process/network | Tokio | Selected features only | `Cargo.toml`; `.meta/project/architecture.md` |
| Config parsing | `serde`, `serde_json`, `json5`, `indexmap` | JSONC-compatible parsing; ordered `features` map | `Cargo.toml`; `.meta/project/architecture.md` |
| Docker integration | Docker CLI subprocesses | Docker required for real build/run/exec/stop workflows | `README.md`; `src/docker.rs` |
| Testing | Rust unit and integration tests | `cargo test`; `proptest` and `tempfile` dev dependencies | `Cargo.toml`; `tests/` |
| CI | GitHub Actions | fmt, clippy, test, build on `main` pushes and PRs | `.github/workflows/ci.yml` |

## Critical Implementation Rules

- Treat `dcc` as a single binary crate; there is no library layer planned.
- Use `anyhow::Result<T>` and add context at meaningful fallible boundaries.
- Keep modules organized by feature area with a shallow hierarchy.
- Prefer project newtypes where primitives carry domain meaning.
- Do not use `unwrap()` or `expect()` outside tests except for documented invariants.
- Do not introduce dependencies when existing dependencies or the standard library cover
  the need.
- Preserve the distinction between build-time `containerEnv` and runtime `remoteEnv`.
- Preserve profile isolation and durable per-profile cache behavior under `.dcc/<profile>`.
- Do not change release, push, or publication behavior without explicit owner direction.

## Conflict-Prone Decisions

| Topic | Decision | Where To Look | Review Trigger |
| --- | --- | --- | --- |
| Crate shape | Single binary crate, no library layer. | `.meta/project/architecture.md` | Introducing public library APIs or cross-crate reuse. |
| Error handling | Use `anyhow` in the binary and context-rich propagation. | `.meta/project/rust-style.md` | Adding broad error handling, panics, or swallowed failures. |
| Config format | Parse devcontainer configs with `json5`; support JSONC-style comments and trailing commas. | `.meta/project/architecture.md`; `src/config/` | Replacing parsing or adding config fields. |
| Feature ordering | Preserve `features` declaration order with `IndexMap`. | `.meta/project/architecture.md`; `src/features/` | Changing merge, install, or feature metadata behavior. |
| Verification boundary | Full completion requires fmt, clippy, and tests; CI also runs build. | `.meta/project/development.md`; `.meta/project/standards.md` | Finishing any code-changing task. |

## Codebase Map

| Area | Key Paths | Patterns To Follow | Pitfalls |
| --- | --- | --- | --- |
| CLI entry and dispatch | `src/main.rs`, `src/cli.rs` | Keep clap definitions explicit and command dispatch narrow. | Global flags may appear before or after subcommands. |
| Workspace/profile/cache identity | `src/workspace.rs`, `src/profile.rs`, `src/cache.rs` | Use domain newtypes and path-aware tests. | Profile names and paths affect container and image identity. |
| Config resolution | `src/config/` | Preserve cycle detection, merge semantics, variable substitution phases, and strict-mode behavior. | Host-specific variables must not be baked into `containerEnv`. |
| Docker operations | `src/docker.rs`, `src/build.rs`, `src/run.rs`, `src/exec.rs`, `src/stop.rs` | Wrap Docker CLI calls and keep errors diagnosable. | Real Docker commands can create external state; distinguish tests from runtime workflows. |
| In-container supervisor | `src/supervisor.rs` | PID 1 owns lifecycle; keep host↔supervisor protocol patch-stable (decision 0004). | Protocol changes require a minor version bump so the semver gate refuses old images. |
| State seeding | `src/seed.rs` | Hydrate declared state from the image on build; record in `.dcc/<profile>.seed.json`. | Seeding runs without state mounts; never mask Feature- or Dockerfile-installed content. |
| UID remap | `src/uid.rs` | `updateRemoteUserUID` bakes a root RUN remapping the container user to the host uid/gid. | Remap is baked at image build; seeding already sees the remapped `/etc/passwd`. |
| Version compatibility | `src/version.rs` | `dcc.version` label interpreted with semver: equal or patch drift proceeds; major/minor or missing label refuses. | `dcc build` is exempt; runtime commands gate on the label. |
| Devcontainer Features | `src/features/` | Preserve OCI/local feature loading, install order, generated Dockerfile behavior, and metadata labels. | Feature scripts run as root but receive remote-user environment metadata. |
| Tests | `tests/`, inline `#[cfg(test)]` modules | Unit-test internals near code; use integration tests for CLI behavior. | Avoid tests that only assert parser acceptance without behavior. |

## Load Rules

- Read this file before implementation work, then inspect the relevant source and tests.
- Before project coding work, load the product README and framework-owned project
  guidance: `README.md`, `.meta/project/development.md`,
  `.meta/project/rust-style.md`, and `.meta/project/architecture.md`.
- Use `.meta/project/standards.md` for exact verified commands.
- Update this file after architecture changes, major dependency changes, or repeated
  implementation drift.
