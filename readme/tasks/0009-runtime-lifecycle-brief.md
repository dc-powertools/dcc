# T-0009 Brief: Durable Runtime Lifecycle Commands

## Goal

Implement the fifth T-0004 slice: make runtime commands work coherently against
durable and one-shot containers, including explicit `start`, `stop`, `run`, `exec`,
`attach`, active-command bookkeeping, and `--keep` promotion.

## Scope

- Add a durable `dcc start` command that starts the profile container without running a
  foreground user command.
- Add `dcc attach` for an interactive shell-oriented session against an existing or
  newly started container.
- Keep `dcc exec <cmd>` as direct command execution and `dcc run <command-name>` as
  `customizations.dcc.commands` resolution, but route both through the shared runtime
  lifecycle path.
- Add `--keep` / `-k` to `run`, `exec`, and `attach` so one-shot containers can be
  promoted to durable mode and new launches can opt into durability.
- Preserve one-shot behavior when `--keep` is absent: the container stops only after all
  active `dcc`-launched commands have finished.
- Allow concurrent `dcc run` / `dcc exec` commands against the same one-shot container
  without prematurely stopping it.
- Run startup hooks (`postStartCommand`) during startup, and run attach hooks
  (`postAttachCommand`) only for `dcc attach`.
- Keep build-preparation hooks (`onCreateCommand`, `updateContentCommand`,
  `postCreateCommand`) out of ordinary runtime commands.
- Prefer generated controller/wrapper assets already installed by T-0008; extend them
  only as needed for active-command tracking and durability mode.
- Preserve existing state mounts, remote env, Feature metadata, unsafe-runtime gating,
  version warning, debug output, and port-forwarding behavior.

## Non-Goals

- No full `portsAttributes` / `otherPortsAttributes` behavior; T-0010 owns port
  attributes.
- No top-level `runArgs` safe allowlist or unsafe devcontainer runtime arg support;
  T-0010 owns it.
- No final official schema validation fixture sweep or parent T-0004 closure; T-0010
  owns final consistency and release-readiness review.
- No cloud snapshot provider or explicit state reset command.

## Acceptance

- `dcc start` starts a durable profile container and is idempotent for an already-running
  durable container.
- `dcc stop` stops either durable or one-shot profile containers by stable dcc container
  id.
- `dcc exec <cmd>` and `dcc run <command-name>` can run against an existing started
  container, or start a one-shot container when none exists.
- Concurrent one-shot commands do not stop the container until the last active command
  exits.
- `dcc run -k`, `dcc exec -k`, and `dcc attach -k` promote an existing one-shot
  container, or start a durable container when none exists.
- `dcc attach` runs collected `postAttachCommand` hooks before the interactive shell or
  explicit attach command; `dcc exec` and `dcc run` do not run attach hooks by default.
- Focused unit tests cover lifecycle planning/bookkeeping and CLI flag parsing, plus the
  required project checks for the commit.

## Verification Plan

- Unit tests for command-mode planning, one-shot active-command bookkeeping, durable
  promotion, attach shell/default command resolution, and startup/attach hook selection.
- CLI flag tests for `start`, `attach`, and `--keep` / `-k` on `run`, `exec`, and
  `attach`.
- Required checks before commit: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, and `cargo build`.
