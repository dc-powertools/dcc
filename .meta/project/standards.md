# Project Standards

This file extends the shared standards in `.meta/meta/development-standards.md`. It is
the sole canonical command catalog for this project.

## Commands

Record only commands verified by successful execution in this environment.

| Action | Exact Command | Prerequisites | Observed Result | Last Verified |
| --- | --- | --- | --- | --- |
| Toolchain check | `cargo --version` | Cargo on `PATH` | Passed; reported `cargo 1.96.0 (30a34c682 2026-05-25)`. | 2026-07-14 |
| Toolchain check | `rustc --version` | Rust on `PATH` | Passed; reported `rustc 1.96.0 (ac68faa20 2026-05-25)`. | 2026-07-14 |
| Component check | `rustup component list --installed \| rg 'rustfmt\|clippy'` | `rustup` and `rg` on `PATH` | Passed; `rustfmt` and `clippy` components are installed for the active toolchain. | 2026-07-14 |
| Format | `cargo fmt --check` | Rust toolchain with `rustfmt` installed | Passed with no diff. | 2026-08-24 |
| Type check | `cargo check` | Rust toolchain and dependencies available | Passed for `dcc v0.1.0`. | 2026-08-24 |
| Lint | `cargo clippy -- -D warnings` | Rust toolchain with `clippy` installed | Passed with warnings denied. | 2026-08-24 |
| Test suite | `cargo test` | Rust toolchain and dependencies available | Passed; 511 unit tests, 31 runnable CLI flag integration tests with 3 ignored, 9 config error tests, 13 fake-Docker boundary tests, 9 feature command tests, and 32 ignored Docker smoke tests listed without running Docker. | 2026-08-24 |
| Build | `cargo build` | Rust toolchain and dependencies available | Passed for the dev profile. | 2026-08-24 |
| CLI smoke run | `cargo run -- --help` | Rust toolchain and dependencies available | Passed; printed CLI help for `dcc`. | 2026-07-14 |
| Workflow lint | `actionlint .github/workflows/*.yml` | `actionlint` on `PATH` | Passed for every workflow with no findings using `actionlint 1.7.12`. | 2026-08-24 |
| Devcontainer config validation | `sudo devcontainer read-configuration --workspace-folder /workspace --include-merged-configuration --log-level trace > /tmp/dcc-devcontainer-read-configuration.json` | Node.js v20.19.2, npm 9.2.0, `@devcontainers/cli 0.87.0`, Docker 26.1.5, and a running Docker daemon. In this harness, Docker needed `dockerd --iptables=false --storage-driver=vfs --bridge=none --ip-forward=false --ip-masq=false`. | Passed; produced 14,073 bytes of merged configuration for `.devcontainer/devcontainer.json`, including root image, Features, mounts, hooks, workspace mount, and defaulted compatibility fields. | 2026-07-15 |

## Architecture

- `dcc` is a single Rust binary crate; do not add a library layer without a recorded
  architectural decision.
- Modules are organized by feature area and should remain shallow.
- Docker integration is through Docker CLI subprocesses.
- Release builds keep `overflow-checks = true`.

## Code Style

- Follow `.meta/project/rust-style.md` for Rust style.
- Use `anyhow::Result<T>` for binary error handling and add context at fallible
  boundaries.
- Keep implementation private by default; use `pub(crate)` before `pub` when widening
  visibility is needed only inside the crate.
- Avoid `unwrap()`, `expect()`, `todo!()`, `unimplemented!()`, and reachable `panic!()`
  in production code.

## Testing

- Unit tests live beside implementation in `#[cfg(test)]` modules.
- Integration tests live under `tests/` and drive the binary through its public CLI
  behavior.
- Docker-dependent integration tests remain ignored for local and development-container
  `cargo test` runs. CI runs the ignored Docker smoke tests explicitly on a GitHub-hosted
  Ubuntu runner with the host Docker daemon available.
- Use property-based testing where broad input spaces matter, especially config parsing,
  merge behavior, Dockerfile generation, and shell quoting.

## Security And Privacy

- Never log secrets, tokens, or credentials.
- Use explicit checked, saturating, or wrapping arithmetic for untrusted input where
  overflow behavior matters.
- Treat real Docker build, run, exec, and stop commands as external-state operations when
  deciding verification scope.

## UI And Accessibility

- Not applicable; this is a CLI project.

## Documentation

- End-user overview and quick-start material belongs in `README.md`.
- Detailed user-facing usage, configuration, command, lifecycle, and compatibility
  guidance belongs in `docs/index.md`.
- devcontainer Feature package behavior belongs in `docs/features.md`.
- Human-facing maintainer setup, verification, and release guidance belongs in
  `docs/development.md`.
- Stable architecture notes belong in `.meta/project/architecture.md`.
- Agent-facing development workflow and commit policy belongs in
  `.meta/project/development.md`.
- Detailed Rust style rules belong in `.meta/project/rust-style.md`.
- Framework-owned project memory belongs under `.meta/project/` and `.meta/tasks/`.
- Project state must not live in unmanaged sidecar docs. Move durable project guidance
  into a framework-owned path and update the source map.
- Backup or collision files, such as `*.bak.md`, are temporary evidence only. Evaluate
  them, migrate durable guidance into the canonical owner, and remove the leftover file.

## Release

- CI runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and
  `cargo build`, plus ignored Docker smoke tests on GitHub-hosted Ubuntu runners.
- Do not push, publish releases, or run release workflows without explicit owner
  direction.
- **Patch releases must not change the host↔supervisor protocol.** The `dcc.version`
  image label is interpreted with semver compatibility (decision 0004): a CLI
  proceeds against an image whose major and minor match, regardless of patch. The
  protocol — the `dcc-ctl` verbs (`mode`, `stop`, `stop-now`, `wait-ready`), the
  `dcc-exec` registration contract, and exit codes 252/253 — is therefore part of the
  patch-stable surface. Any change to it requires at least a minor version bump so the
  compatibility gate refuses the old image rather than silently driving an
  incompatible supervisor.

## Do Not Do

- Do not silently change nearby behavior outside the requested scope.
- Do not add dependencies without concrete justification.
- Do not replace established docs or command entry points; link them from framework
  records instead.
- Do not work from paths ignored by `.gitignore` unless the task explicitly requires
  inspecting generated, cached, or ignored content.
- Do not use auto-resolving prompts for required Codex user input; when a question blocks
  safe progress, wait for the answer.
