# T-0004 Brief: dcc Devcontainer Compatibility And Lifecycle Rewrite

## Goal

Rewrite `dcc` so project and Feature configuration can validate against the official
devcontainer schema while `dcc` provides local state persistence, deterministic build
preparation, and longer-lived container lifecycle commands.

## Scope

- Move `dcc`-specific configuration under `customizations.dcc`.
- Add `customizations.dcc.state` as a list of stateful container paths.
- Merge `state` values across Feature metadata and devcontainer config.
- Replace explicit cache/mount declarations in Features and project configs with state
  path declarations.
- For local containers, mount each state path from the profile cache before stateful
  lifecycle initialization so those commands populate the persistent cache directly.
- Add durable container lifecycle commands, including `start`, `stop`, `run`, `exec`, and
  shell-oriented `attach`.
- Keep one-shot command execution available, with a `--keep`/`-k` style durable flag.

## Current Decisions

- The schema-compatible namespace is `customizations.dcc.state`, not
  `customizations.state`.
- `state` is a list.
- Local persistence uses host cache paths mounted at each state path before lifecycle
  initialization commands run.
- Cloud snapshot support is out of scope for this implementation, but the architecture
  should leave room for future snapshot providers.
- `postAttachCommand` represents an attachment payload, not a hidden prelude before every
  command.
- `dcc start` does not run `postAttachCommand`.
- `dcc attach` runs collected `postAttachCommand` hooks in sequence, then starts an
  interactive shell if those hooks complete. A blocking attach hook may intentionally
  occupy the session.
- `dcc exec <cmd>` does not run `postAttachCommand` by default.
- Official devcontainer CLI configuration reading/validation is an acceptable acceptance
  target for schema compatibility.
- Likely schema/config validation command: official devcontainer CLI
  `read-configuration --workspace-folder <fixture> --include-merged-configuration`; the
  CLI's own tests use this command for configuration reading, while Feature collections
  use `devcontainer features test`.
- `customizations.dcc.state` entries should resolve to absolute container paths; allow
  supported container-side variables, reject relative paths, duplicate normalized paths,
  and unresolved values.
- Keep `extends`, but move it under `customizations.dcc.extends`.
- Support legacy top-level `extends` for a brief transition period with a deprecation
  warning; new docs, fixtures, and tests use `customizations.dcc.extends`.
- Continue to parse official `mounts`, including full official syntax, but do not use
  explicit mounts as the state/cache persistence mechanism.
- Support official `forwardPorts` numbers and string forms.
- Lifecycle commands should follow official collection/ordering behavior.
- Keep the `containerUser` default of `dev` unless the image already has a user or
  `containerUser` is explicitly specified.
- Move top-level `scripts` to `customizations.dcc.commands`.
- Support legacy top-level `scripts` for a brief transition period with a deprecation
  warning; new docs, fixtures, and tests use `customizations.dcc.commands`.
- Preserve existing command resolution semantics for `customizations.dcc.commands`: feature
  commands are addressed as `<feature-id>:<command>`, devcontainer commands as
  `:<command>`, and unqualified names are accepted only when they are unique.
- Keep feature command prefix fallback behavior when metadata lacks a clean ID, but warn
  because full-reference command names are awkward. Consider shorthand such as
  `:2:foo` for the second canonical feature-provided `foo` command, with error/list
  output showing the full feature reference the shorthand maps to.
- Keep `containerEnv` and `remoteEnv` behavior strict and explicit; only change behavior
  where a specific official rule is clear and valuable.
- Parse full `portsAttributes` / `otherPortsAttributes`; implement `label`, `protocol`,
  and `onAutoForward` values `openBrowser`, `openBrowserOnce`, `openPreview` (same best
  effort as browser), `silent`, and `ignore`. Do not implement privilege elevation.
- Parse `runArgs`; allow a strict safe subset, reject privileged/security-sensitive args
  unless the user passes an explicit unsafe-args approval flag.
- Parse `overrideCommand` for schema compatibility but do not let it disable `dcc`'s
  managed keepalive behavior.
- Support `workspaceFolder` behaviorally. Warn during build/config processing when it is
  not under `${containerWorkspaceFolder}/`; commands and lifecycle hooks use it as the
  workdir.
- Parse but do not support `workspaceMount`; `dcc` owns workspace mounting. Document this
  as an intentional behavior difference.
- User-specified `mounts` remain parsed and locally supported for compatibility, but they
  are not the state/cache mechanism and may be restricted in future cloud/container
  deployment modes because arbitrary host mounts are not portable.
- Local state cache identity is profile plus normalized container path; rebuilding an
  image never wipes local state.
- State strings are directory entries. Object entries may declare file state, e.g.
  `{ "path": "${containerEnv:HOME}/.npmrc", "type": "file" }`.
- There is no local seed snapshot for state. Features and devcontainer hooks populate
  stateful directories/files directly while those paths are mounted from the local cache.
- Reject state paths that are unresolved, relative, include `..`, are `/`, overlap as
  parent/child entries, or target runtime/system areas such as `/tmp`, `/run`, `/proc`,
  `/sys`, `/dev`, or reserved `/workspace/.dcc`.
- Newly created state paths should be owned for the effective command user. If the path
  already exists in the image/container, preserve or mirror its existing access rules
  rather than forcibly chowning it to the user.
- Build flags such as `--no-cache` or `--update` must not reset local state. State reset
  should be explicit future functionality.
- Known edge case: features that install mutable remote binaries into state may leave
  stale cached binaries after a rebuild. Accept this for now; the long-term fix is
  version-aligned feature packages.
- For local execution, `onCreateCommand`, `updateContentCommand`, and
  `postCreateCommand` should not run implicitly during ordinary `dcc run` / `dcc exec`.
  They belong to an explicit build/update/preparation phase.
- `dcc build` performs preparation by default because build without prepare is rare:
  run `onCreateCommand`, `updateContentCommand`, and `postCreateCommand` with state
  mounts attached after the image is available.
- Add `dcc build --refresh-only` to skip image rebuild and skip `onCreateCommand`, then
  run only `updateContentCommand` and `postCreateCommand` for incremental refresh.
- Do not track `onCreateCommand` run-once state in metadata. `dcc build` always runs it
  unless `--refresh-only` is specified, favoring deterministic command behavior over
  stateful drift-prone tracking.
- `dcc build --refresh-only` fails clearly when the profile image does not already exist.
- Concurrent `dcc run` / `dcc exec` commands are allowed without `-k`.
- A one-shot container should stop only after all active `dcc`-launched commands finish.
- Running `dcc run -k`, `dcc exec -k`, or `dcc attach -k` against an existing one-shot
  container promotes it to durable mode.
- Container mode and active-command bookkeeping should not rely solely on Docker labels,
  because labels cannot be changed after container creation.
- Inject a small `dcc` controller/supervisor and launch user commands through a
  `dcc-command` wrapper so PID 1 owns active-command tracking and one-shot shutdown.
- The in-container controller should own startup hook execution and readiness. The
  `dcc-command` wrapper may be invoked while startup is in progress and must block or
  queue until startup hooks complete.
- Implement the controller/wrapper initially as small POSIX shell scripts plus generated
  config data. Re-evaluate embedding a Rust controller if shell complexity grows.
- Materialize lifecycle hooks as generated scripts in the image; controller scripts run
  sorted startup hook files, and `dcc attach` runs sorted attach hook files before the
  interactive shell.
- `dcc attach` default shell resolution: use executable absolute `$SHELL` when available,
  else `/bin/bash`, else `/bin/sh`; allow explicit shell/command override.
- Lifecycle hook order for each phase: feature hooks in canonical feature/install order,
  then devcontainer config-chain hooks from earliest `customizations.dcc.extends`
  ancestor to the terminal invoked profile config. Object-form hook entries run in
  parallel and block until complete.
- Merge behavior is property-specific and may be codified even for properties that are
  parsed but not behaviorally supported. Behavioral support decisions are separate from
  merge rules.
- `build` merge behavior is whole-object last-wins.
- Warn when two `forwardPorts` entries map a local/container port relationship
  differently.
- When a profile config inherits `name` from an ancestor and does not override it, append
  the profile slug in parentheses to the implicit display/container name, e.g.
  `foo (node)` for `node.json` extending a config named `foo`.
- The inherited-name profile suffix affects display/Docker-visible container naming only;
  stable `ContainerId` and image tag identity remain based on workspace identity plus
  profile.
- Support official `build` as an alternative base source to `image`; fail clearly when
  both `image` and `build` are set for a profile.
- `dcc` managed containers start PID 1/controller as root, overriding image `USER`.
- Parse `containerUser` and `remoteUser`, but error if both are specified and differ.
  Resolved user-dependent state paths should be based on the runtime command user, not a
  separate build-time container user.
- Feature-provided `containerUser` and `remoteUser` are schema validation errors because
  the Feature schema does not permit them.
- Feature privilege-escalation properties such as `privileged`, `capAdd`, and
  `securityOpt` use the same explicit unsafe approval mechanism as equivalent
  devcontainer/runtime properties.
- Feature `init` and `entrypoint` are parsed but behaviorally unsupported for now; build
  emits warnings when they appear because `dcc` owns PID 1/controller startup.
- Unsafe runtime settings are rejected by default and require an explicit
  `--allow-unsafe-runtime` flag for the current invocation/profile. This applies to
  Feature/devcontainer privilege properties (`privileged`, `capAdd`, `securityOpt`) and
  unsafe `runArgs` such as `--privileged`, `--cap-add`, `--security-opt`, `--pid=host`,
  `--ipc=host`, `--network=host`, `--device`, and sensitive host mounts.
- `mounts` safety: continue supporting normal project-local/local-cache mounts; warn or
  reject sensitive host mounts such as Docker socket, `/`, `/etc`, `/var/run`, and SSH
  agent sockets unless `--allow-unsafe-runtime` is present.

## Implementation Process Notes

- This task is large and should be delivered in self-contained, reviewable slices with
  periodic task-note checkpoints and local commits for completed slices.
- Add follow-on tasks for independent discoveries instead of silently expanding scope.
- Keep framework state, user-facing `README.md`, and project architecture docs current as
  implementation decisions solidify.
- Monitor usage occasionally; a single reset is acceptable if needed to continue after
  usage exceeds 95% of the limit.
- `initializeCommand` is parsed for schema compatibility but behaviorally unsupported by
  default because it runs on the host and can execute repository-supplied code outside
  the container trust boundary.
- `waitFor` is parsed but ignored behaviorally for now. `dcc build` waits for all
  build-prep hooks (`onCreateCommand`, `updateContentCommand`, `postCreateCommand`) to
  finish; startup and attach hooks also block their respective phases.

## Open Questions

- Exact safe allowlist for `runArgs` and mount forms.
- Whether `initializeCommand` belongs in `dcc build`, `dcc start`, both, or neither.
- `waitFor` behavior now that `dcc build` runs build-prep hooks synchronously.
- State path normalization details for parent/child overlap and file mount creation.
- Controller failure reporting and timeout behavior.

## Acceptance Draft

- Example configs using `customizations.dcc.state` validate with the official devcontainer
  CLI schema validator.
- Existing supported `dcc` config behavior either migrates to official schema fields or
  is documented as unsupported/different.
- State paths are mounted from local profile cache during build preparation and runtime,
  so lifecycle initialization populates persistent local state directly.
- `dcc start`, `dcc stop`, `dcc run`, `dcc exec`, and `dcc attach` work coherently for
  stopped, running one-shot, and running durable containers.
- Required Rust checks pass: `cargo fmt --check`, `cargo clippy -- -D warnings`,
  `cargo test`, and `cargo build`.
