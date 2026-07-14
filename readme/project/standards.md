# Project Standards

This file extends the shared standards in `readme/meta/development-standards.md`. It is
the sole canonical command catalog for this project.

## Commands

Record only commands verified by successful execution in this environment.

| Action | Exact Command | Prerequisites | Observed Result | Last Verified |
| --- | --- | --- | --- | --- |
| Toolchain check | `cargo --version` | Cargo on `PATH` | Passed; reported `cargo 1.96.0 (30a34c682 2026-05-25)`. | 2026-07-14 |
| Toolchain check | `rustc --version` | Rust on `PATH` | Passed; reported `rustc 1.96.0 (ac68faa20 2026-05-25)`. | 2026-07-14 |
| Component check | `rustup component list --installed \| rg 'rustfmt\|clippy'` | `rustup` and `rg` on `PATH` | Passed; `rustfmt` and `clippy` components are installed for the active toolchain. | 2026-07-14 |
| Format | `cargo fmt --check` | Rust toolchain with `rustfmt` installed | Passed with no diff. | 2026-07-14 |
| Type check | `cargo check` | Rust toolchain and dependencies available | Passed for `dcc v0.0.33`. | 2026-07-14 |
| Lint | `cargo clippy -- -D warnings` | Rust toolchain with `clippy` installed | Passed with warnings denied. | 2026-07-14 |
| Test suite | `cargo test` | Rust toolchain and dependencies available | Passed; 385 unit tests, 19 runnable CLI flag integration tests with 2 ignored, and 9 config error integration tests passed. | 2026-07-14 |
| Build | `cargo build` | Rust toolchain and dependencies available | Passed for the dev profile. | 2026-07-14 |
| CLI smoke run | `cargo run -- --help` | Rust toolchain and dependencies available | Passed; printed CLI help for `dcc`. | 2026-07-14 |

## Architecture

- `dcc` is a single Rust binary crate; do not add a library layer without a recorded
  architectural decision.
- Modules are organized by feature area and should remain shallow.
- Docker integration is through Docker CLI subprocesses.
- Release builds keep `overflow-checks = true`.

## Code Style

- Follow `readme/project/rust-style.md` for Rust style.
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

- Product-facing behavior belongs in `README.md`.
- Stable architecture notes belong in `readme/project/architecture.md`.
- Detailed development workflow belongs in `readme/project/development.md`.
- Detailed Rust style rules belong in `readme/project/rust-style.md`.
- Framework-owned project memory belongs under `readme/project/` and `readme/tasks/`.
- Project state must not live in unmanaged sidecar docs. Move durable project guidance
  into a framework-owned path and update the source map.
- Backup or collision files, such as `*.bak.md`, are temporary evidence only. Evaluate
  them, migrate durable guidance into the canonical owner, and remove the leftover file.

## Release

- CI runs `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and
  `cargo build`.
- Do not push, publish releases, or run release workflows without explicit owner
  direction.

## Do Not Do

- Do not silently change nearby behavior outside the requested scope.
- Do not add dependencies without concrete justification.
- Do not replace established docs or command entry points; link them from framework
  records instead.
- Do not work from paths ignored by `.gitignore` unless the task explicitly requires
  inspecting generated, cached, or ignored content.
- Do not use auto-resolving prompts for required Codex user input; when a question blocks
  safe progress, wait for the answer.
