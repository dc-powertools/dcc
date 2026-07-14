# T-0004 Notes: Implementation Checkpoints

## 2026-07-14 Clarification Checkpoint

The rewrite is ready to enter implementation. The brief owns the durable requirements:
`readme/tasks/0004-dcc-rewrite-brief.md`.

Key settled directions:

- Schema-compatible `dcc` config lives under `customizations.dcc`.
- Legacy top-level `extends` and `scripts` remain temporarily supported with deprecation
  warnings, but new docs/tests use `customizations.dcc.extends` and
  `customizations.dcc.commands`.
- `state` is `customizations.dcc.state`; strings mean directories, object entries can
  declare `{ "path": "...", "type": "file" }`.
- Local state has no seed snapshot. Build preparation mounts local state paths and hooks
  populate them directly.
- `dcc build` performs preparation by default: `onCreateCommand`,
  `updateContentCommand`, then `postCreateCommand`. `--refresh-only` skips image rebuild
  and `onCreateCommand`; it fails if the image is missing.
- `initializeCommand`, `waitFor`, `workspaceMount`, Feature `init`, Feature `entrypoint`,
  and `overrideCommand` are parsed for compatibility but unsupported or ignored with
  clear warnings as documented in the brief.
- Managed containers run a root PID 1 controller. User hooks/commands run as the resolved
  runtime user. `containerUser` and `remoteUser` may both be specified only if identical.
- The controller and `dcc-command` wrapper are initially generated shell scripts; revisit
  a Rust controller if shell complexity grows.
- `dcc start`, `run`, `exec`, `attach`, and `stop` share one profile container. One-shot
  containers stop only after all `dcc` commands finish; `-k` promotes to durable.
- Feature and devcontainer privilege escalation requires `--allow-unsafe-runtime`.
- Official config validation should use the Dev Container CLI
  `read-configuration --workspace-folder <fixture> --include-merged-configuration`
  where available.

Suggested implementation order:

1. Config model, parser, merge chain, `customizations.dcc`, and compatibility warnings.
2. State path model and local cache layout.
3. Feature metadata changes: state/commands, ignored or unsafe Feature properties, hook
   collection order.
4. Build pipeline: image source (`image`/`build`), generated controller/hook files,
   default preparation and `--refresh-only`.
5. Runtime controller integration: `start`, `run`, `exec`, `attach`, `stop`, `-k`.
6. Port attributes, safe `runArgs`, docs, fixtures, validation command integration.

Checkpoint rule:

- Commit completed slices independently after required checks pass.
- Add follow-on tasks for independent discoveries that are not required for the current
  slice.

## 2026-07-14 Decomposition Checkpoint

User direction added full-framework execution, sub-agents, strict review, a work queue,
regular self-contained commits, and usage monitoring. T-0004 is now the parent
initiative; child tasks T-0005 through T-0010 own implementation slices and commit
boundaries.

Current queue:

- T-0005: config/schema compatibility under `customizations.dcc`.
- T-0006: validated state path model and cache mount planning.
- T-0007: Feature metadata, commands/state/hooks, unsupported and unsafe settings.
- T-0008: build preparation, official `build`, generated controller assets, and
  `--refresh-only`.
- T-0009: durable runtime `start`/`stop`/`run`/`exec`/`attach`, one-shot bookkeeping,
  and `--keep` promotion.
- T-0010: port attributes, safe `runArgs`, docs, fixtures, validation, final review,
  and parent closure.

Worker and usage plan:

- Root Orchestrator owns implementation integration and all shared documentation.
- Sub-agents may do bounded read-only architecture, QA, security, and review work, or
  code edits only with disjoint write ownership.
- No repo `codex-quota-monitor` skill is installed or discoverable in this session.
  Required authoritative usage telemetry is unavailable, so child concurrency is kept
  conservative and should pause if telemetry becomes available and reports either
  applicable window at or above 95%.
