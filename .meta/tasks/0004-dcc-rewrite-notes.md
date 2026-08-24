# T-0004 Notes: Implementation Checkpoints

## 2026-07-14 Clarification Checkpoint

The rewrite is ready to enter implementation. The brief owns the durable requirements:
`.meta/tasks/0004-dcc-rewrite-brief.md`.

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

Worker plan:

- Root Orchestrator owns implementation integration and all shared documentation.
- Sub-agents may do bounded read-only architecture, QA, security, and review work, or
  code edits only with disjoint write ownership.

## 2026-07-14 T-0005 Completion Checkpoint

T-0005 implemented the config/schema compatibility slice in the root session, with
sub-agents used for architecture audit, QA planning, and read-only review. The user
flagged that implementation-heavy work should be delegated more aggressively to protect
root orchestration context. Starting with T-0006, implementation-heavy slices should go
to worker agents with explicit disjoint ownership while the root session handles
planning, integration, state updates, and final review.

T-0005 observed checks:

- `cargo fmt --check`: passed.
- `cargo clippy -- -D warnings`: passed.
- `cargo test`: passed; 337 unit tests, 16 runnable CLI flag tests with 2 ignored, and
  9 config error integration tests.
- `cargo build`: passed.

Reviewer result: read-only specialist review found no blocking findings. Follow-up
risks were moved to the quality record: T-0006 must handle same-path conflicting state
types, runtime command integration remains later, and raw merged `dcc.extends` should be
kept irrelevant or cleaned before any later code inspects merged raw config directly.

Pause checkpoint: the user requested a pause after T-0005 so they can clear context and
prepare a resume. Scheduling is paused with T-0006 Ready. On resume, first set
Scheduling back to Running, create a T-0006 brief, and delegate the implementation-heavy
state/cache work to a worker agent with explicit file ownership.

## 2026-07-14 T-0006 Completion Checkpoint

T-0006 implemented validated project state paths and profile-local state mount
planning. A worker handled the Rust patch; the root session reviewed and tightened
state substitution so host-local variables remain invalid in container state paths.

Implemented behavior:

- `customizations.dcc.state` is validated after config merge and container-side
  substitution.
- String entries remain directory state; object entries can declare file state.
- Compatible duplicate normalized paths deduplicate; conflicting kinds and parent/child
  overlaps error.
- Invalid unresolved, relative, root, `..`, runtime/system, and `/workspace/.dcc` paths
  error.
- `${containerEnv:VAR}` state paths are deferred until runtime environment probing, then
  revalidated.
- Runtime execution plans state bind mounts under `.dcc/<profile>/state/...`, prepares
  directory/file host sources, and includes state mounts in debug output.

T-0006 observed checks:

- `cargo fmt --check`: passed.
- `cargo clippy -- -D warnings`: passed.
- `cargo test`: passed; 355 unit tests, 16 runnable CLI flag tests with 2 ignored, and
  9 config error integration tests.
- `cargo build`: passed.

Residual risk: no live Docker smoke test was run in this slice; durable lifecycle and
build-preparation behavior remain owned by T-0008 and T-0009.

## 2026-07-14 T-0007 Completion Checkpoint

T-0007 implemented Feature metadata compatibility and unsafe Feature runtime gating. A
worker handled the Rust patch; the root session reviewed it, added build-time
cross-Feature state overlap validation, ran the full checks, and updated documentation.

Implemented behavior:

- Feature `customizations.dcc.commands` is serialized through `devcontainer.metadata`
  and contributes to `dcc run` command listing/resolution.
- Legacy Feature top-level `scripts` remains supported with a deprecation warning;
  nested commands win on conflicts.
- Feature `customizations.dcc.state` is validated, serialized, parsed from metadata,
  merged before project state, and planned as profile-local state mounts.
- Feature lifecycle hook order remains Feature installation order before project hooks.
- Feature `init` and `entrypoint` warn and are ignored.
- Feature `containerUser` and `remoteUser` error.
- Feature `privileged`, `capAdd`, and `securityOpt` require `--allow-unsafe-runtime` at
  build and runtime; allowed settings become Docker runtime args.

T-0007 observed checks:

- `cargo fmt --check`: passed.
- `cargo clippy -- -D warnings`: passed.
- `cargo test`: passed; 371 unit tests, 17 runnable CLI flag tests with 2 ignored, and
  9 config error integration tests.
- `cargo build`: passed.

Residual risk: top-level devcontainer `runArgs` and sensitive mount allowlisting remain
owned by T-0010; no live Docker smoke test was run in this slice.

## 2026-07-14 T-0008 Completion Checkpoint

T-0008 implemented official build-source support and build preparation. A worker handled
the Rust patch; the root session reviewed the implementation, ran the full checks, and
updated documentation and framework records.

Implemented behavior:

- Official top-level `build` is parsed as an alternative to `image`; setting both errors
  clearly.
- Dockerfile/context build sources are built as a generated base image, then `dcc` builds
  its managed stage from that base.
- Generated controller, command-wrapper, and build-preparation hook assets are included
  in the build context.
- `dcc build` runs `onCreateCommand`, `updateContentCommand`, and `postCreateCommand`
  after the image is available, with Feature hooks before project hooks for each phase
  and state mounts attached.
- `dcc build --refresh-only` skips image rebuild and `onCreateCommand`, requires the
  profile image to exist, and runs only update/post-create hooks.
- `--no-cache` and `--update` do not reset local state.

T-0008 observed checks:

- `cargo fmt --check`: passed.
- `cargo clippy -- -D warnings`: passed.
- `cargo test`: passed; 385 unit tests, 19 runnable CLI flag tests with 2 ignored, and
  9 config error integration tests.
- `cargo build`: passed.

Residual risk: no live Docker smoke test was run in this slice. Durable runtime command
behavior remains owned by T-0009.

## 2026-07-14 T-0009 Completion Checkpoint

T-0009 implemented durable runtime lifecycle commands. A worker handled the main Rust
patch; the root session reviewed it, fixed macOS-compatible active-command liveness
checking, ran the full checks, and updated documentation and framework records.

Implemented behavior:

- `dcc start` starts a durable profile container and is idempotent for an already-running
  profile container.
- `dcc run`, `dcc exec`, and `dcc attach` reuse an existing profile container, or start
  a one-shot container when none exists.
- `--keep` / `-k` on `run`, `exec`, and `attach` starts durable mode or promotes an
  existing one-shot container.
- One-shot active-command bookkeeping lives under `.dcc/<profile>/runtime`; a one-shot
  container stops only after all active `dcc`-launched commands finish.
- Runtime hooks are scoped by phase: startup runs `postStartCommand`; attach runs
  `postAttachCommand`; ordinary runtime commands do not run build-preparation hooks.
- `dcc attach` defaults to an interactive shell resolution script, with explicit attach
  command arguments still supported.
- `dcc stop` clears host-side runtime bookkeeping after stopping the profile container.

T-0009 observed checks:

- `cargo fmt --check`: passed.
- `cargo clippy -- -D warnings`: passed.
- `cargo test`: passed; 392 unit tests, 21 runnable CLI flag tests with 2 ignored, and
  9 config error integration tests.
- `cargo build`: passed.

Residual risk: no live Docker smoke test was run in this slice. Runtime mode and
active-command state are host-side cache records rather than PID-1-owned controller
state. `dcc start` does not leave a background host-side port-forwarding process behind;
foreground `run`, `exec`, and `attach` invocations still own their relay tasks.

## 2026-07-14 T-0010 Completion And Parent Closure Checkpoint

T-0010 completed the final compatibility, validation, and review slice. A worker handled
the parser/runtime implementation; the root session reviewed it, ran checks, requested a
read-only security review, fixed the blocking mount-normalization finding, updated docs
and records, and closed parent T-0004 with residual risks recorded.

Implemented behavior:

- Top-level `portsAttributes` and `otherPortsAttributes` parse `label`, `protocol`, and
  `onAutoForward` values `openBrowser`, `openBrowserOnce`, `openPreview`, `silent`, and
  `ignore`. Browser/preview auto-open behavior is not implemented.
- Top-level `runArgs` parses and passes a conservative safe Docker runtime flag subset.
  Unknown flags are rejected.
- Top-level `privileged`, `capAdd`, `securityOpt`, unsafe `runArgs`, and sensitive mounts
  require `--allow-unsafe-runtime`.
- Sensitive mount checks cover non-normalized parent-directory source paths, Docker
  sockets, `/`, `/etc`, `/var/run`, SSH paths, and SSH-agent-like mount targets.
- Cache mount source auto-creation rejects parent-directory source paths before creating
  anything under the host filesystem.
- `overrideCommand` and `workspaceMount` parse for schema compatibility and warn because
  `dcc` owns keepalive startup and workspace mounting.
- `workspaceFolder` is used as the build-prep hook, runtime hook, and foreground command
  workdir; it warns when outside `/workspace`.

T-0010 observed checks after review fixes:

- `cargo fmt --check`: passed.
- `cargo clippy -- -D warnings`: passed.
- `cargo test`: passed; 406 unit tests, 23 runnable CLI/config tests with 2 ignored, and
  9 config error integration tests.
- `cargo build`: passed.

2026-07-15 follow-up validation: after installing Node.js v20.19.2, npm 9.2.0,
`@devcontainers/cli 0.87.0`, and Docker 26.1.5, the official CLI check passed:

- `sudo devcontainer read-configuration --workspace-folder /workspace --include-merged-configuration --log-level trace`
- A constrained local Docker daemon was started with the `vfs` storage driver and no
  bridge/iptables because the harness container does not expose the default Docker
  daemon capabilities.
- The command produced `/tmp/dcc-devcontainer-read-configuration.json`, a 14,073-byte
  merged configuration including the root `debian` image, configured Features, Feature
  metadata, mounts, hooks, workspace mount, and defaulted compatibility fields.
- Live Docker smoke was attempted with
  `cargo test --test cli_flags strict_accepts_valid_config -- --ignored --exact --nocapture`.
  The constrained daemon allowed `docker ps` and official CLI inspection but could not
  build images in this harness: BuildKit failed to mount the build context with
  `operation not permitted`, and `DOCKER_BUILDKIT=0` failed with `unshare: operation not
  permitted`. No containers or images were left behind.

Residual risks accepted for parent closure:

- Live `dcc` Docker smoke testing remains blocked for final durable
  lifecycle/build-prep behavior until validation runs on a Docker host with mount and
  namespace capabilities.
- Port attributes are parsed for compatibility, but browser/preview auto-open behavior is
  not implemented.
- Mutable runtime mode and active-command state remain host-cache records rather than
  PID-1-owned controller state.
