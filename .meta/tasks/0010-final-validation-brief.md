# T-0010 Brief: Final Compatibility, Validation, And Parent Closure

## Goal

Complete the final T-0004 slice: fill the remaining devcontainer compatibility gaps,
document the final runtime behavior, run strict review/validation, and prepare T-0004
for closure.

## Scope

- Parse `portsAttributes` and `otherPortsAttributes`; support the values needed by
  local behavior and documentation:
  - `label`
  - `protocol`
  - `onAutoForward`: `openBrowser`, `openBrowserOnce`, `openPreview`, `silent`, and
    `ignore`
- Parse top-level `runArgs`; pass a strict safe subset through to Docker runtime
  commands and reject privileged/security-sensitive args unless
  `--allow-unsafe-runtime` is present.
- Extend unsafe runtime gating to devcontainer-provided runtime properties, not only
  Feature metadata.
- Review user `mounts` safety and reject or gate sensitive host mounts such as the Docker
  socket, `/`, `/etc`, `/var/run`, and SSH agent sockets unless
  `--allow-unsafe-runtime` is present.
- Parse `overrideCommand` for schema compatibility, but do not let it disable `dcc`'s
  managed keepalive/controller startup.
- Support `workspaceFolder` as the runtime workdir, warning when it is not under
  `${containerWorkspaceFolder}/`.
- Parse `workspaceMount` for schema compatibility, warn that `dcc` owns workspace
  mounting, and keep behavior unchanged.
- Update README, architecture docs, threat model, quality record, fixtures/tests, and the
  task catalog to reflect final behavior.
- Run the official devcontainer configuration validation target when available, or record
  the exact environment blocker.
- Run strict security/release-readiness review before closing T-0004.

## Non-Goals

- No cloud snapshot provider.
- No state reset command.
- No background daemon for durable `dcc start` port forwarding unless it is necessary to
  fix a blocking regression.
- No release publication, tag, push, or CI workflow changes.

## Acceptance

- Official-schema fields needed for final compatibility are parsed without strict-mode
  unknown-field failures.
- Safe `runArgs` are applied in Docker runtime args; sensitive `runArgs` and mounts are
  rejected by default and allowed only with `--allow-unsafe-runtime`.
- Runtime commands use `workspaceFolder` as their workdir when configured.
- `workspaceMount` and unsupported compatibility fields produce clear warnings or
  documented behavior differences.
- Project docs and quality records match the implemented behavior.
- Required checks pass: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`,
  and `cargo build`.
- Official devcontainer validation is run or a precise blocker is recorded.
- Parent T-0004 can be closed or has only explicitly recorded residual risks.

## Verification Plan

- Unit tests for parsing/merge behavior, safe/unsafe `runArgs`, sensitive mount gating,
  workspaceFolder workdir planning, and port attribute parsing.
- CLI/config integration tests for strict-mode acceptance and unsafe rejection messages.
- Official validation command:
  `devcontainer read-configuration --workspace-folder <fixture> --include-merged-configuration`
  when the devcontainer CLI is available.
- Required checks before commit: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, and `cargo build`.
