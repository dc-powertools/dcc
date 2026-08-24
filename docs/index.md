# User Guide

This guide explains the user-facing behavior behind `dcc` profiles, state,
commands, lifecycle hooks, devcontainer compatibility, and supported
configuration.

For package-based devcontainer Features installed through the top-level
`features` field, see [Devcontainer Features](features.md).

## Profiles

`dcc` enables many profile-specific environments called profiles. The default
profile is represented by `.devcontainer/devcontainer.json`. Passing
`-p/--profile <name>` loads `.devcontainer/<name>.json`.

`dcc` searches for the `.devcontainer` directory by walking up from the current
working directory through its ancestors, stopping at the first directory that
contains a `.devcontainer` directory. You can run `dcc` from any subdirectory of
a project.

Standalone `.devcontainer.json` files are not supported. All profile
configurations must be located within `.devcontainer/`.

Each profile has its own image, container identity, durable cache, and declared
state. The default is isolation between profiles.

## Config Inheritance

A devcontainer config file may use `customizations.dcc.extends` to inherit all
properties from another local file. This allows multiple profiles to layer small
changes on top of a common base configuration.

`dcc` generally follows the outline of the proposal in
[devcontainers/spec#22](https://github.com/devcontainers/spec/issues/22).
Arrays and objects are combined as a union of values, while basic types are
overwritten. Lifecycle command fields are not merged; the child value wins.

The path given in `extends` is resolved relative to the file that contains it.
Extension chains are permitted. Circular chains are invalid and cause
`dcc build` to exit with an error.

```jsonc
// .devcontainer/base.json
{
  "name": "example/project",
  "forwardPorts": [80, 5432],
  "containerEnv": {
    "CARGO_HOME": "/cache/cargo",
    "RUST_BACKTRACE": "0"
  }
}

// .devcontainer/derived.json
{
  "customizations": {
    "dcc": {
      "extends": "./base.json"
    }
  },
  "forwardPorts": [80, 2222],
  "containerEnv": {
    "RUST_BACKTRACE": "1"
  },
  "onCreateCommand": "echo hello"
}
```

The derived profile resolves to the union of forwarded ports
`[80, 5432, 2222]`, keeps `CARGO_HOME`, overwrites `RUST_BACKTRACE`, and uses the
child `onCreateCommand`.

Legacy top-level `extends` is still accepted with a warning. New configs should
use `customizations.dcc.extends`.

## Durable Cache

Every profile gets a durable cache directory mounted in the container at
`/cache`. The host cache lives under `.dcc/<profile>` in the workspace.

These variables are available in devcontainer configuration:

| Variable | Where It Can Be Used | Description |
| --- | --- | --- |
| `${localCacheFolder}` | Host-side fields such as `remoteEnv` and `mounts` | Path of the local profile cache folder. |
| `${containerCacheFolder}` | Container-side fields | Path of the cache folder in the container, always `/cache`. |

For tool caches that can live under `/cache`, set an environment variable:

```jsonc
"containerEnv": {
  "CARGO_HOME": "${containerCacheFolder}/cargo"
}
```

For arbitrary Docker mount syntax, use `mounts`:

```jsonc
"mounts": [
  "type=bind,src=${localCacheFolder}/target,dst=/workspace/target"
]
```

Runtime launches automatically create the host-side source directory for any
bind mount whose source path lies under `${localCacheFolder}`.

The container workspace directory is always `/workspace`. The
`/workspace/.dcc` subdirectory is masked inside the container by an empty tmpfs
mount so profile state does not leak into the container workspace.

## Declared State

The preferred way to preserve tool state is to declare container paths under
`customizations.dcc.state`. String entries are directories; object entries can
declare file state:

```jsonc
"customizations": {
  "dcc": {
    "state": [
      "/home/dev/.cargo",
      { "path": "/home/dev/.npmrc", "type": "file" }
    ]
  }
}
```

Each state path is mounted from `.dcc/<profile>/state/...` on the host. State
paths must be absolute container paths. `${containerWorkspaceFolder}`,
`${containerCacheFolder}`, and `${containerEnv:VAR}` are supported. Host-local
variables such as `${localCacheFolder}` and `${localEnv:VAR}` are rejected.

`dcc` also rejects root, relative, overlapping, unresolved, and critical paths.
The critical-path guards are hard rejects and apply both at config load and
after `${containerEnv:VAR}` resolution.

Whole subtrees are blocked for paths such as `/proc`, `/sys`, `/dev`, `/tmp`,
`/run`, `/bin`, `/sbin`, `/etc`, `/workspace/.dcc`, `/cache`, and
`/usr/local/share/dcc`. Exact path only guards apply to broad roots such as
`/usr`, `/var`, `/home`, `/root`, `/opt`, `/workspace`, `/srv`, `/mnt`, and
`/media`; specific subdirectories such as `/home/dev/.cargo` and
`/workspace/target` remain valid.

These guards are textual. A state path that is a symlink in the image resolving
outside itself is not detected here.

## Seeding State From The Image

Bind mounts never copy image content into an empty host source. Without seeding,
data the image build places at a declared state path would be silently masked by
an empty directory or file.

`dcc build` seeds declared state from the image. It runs one short-lived
container on the finished image with state mounts disabled and the host state
root mounted at `/dcc-seed`, then copies each declared path's image content into
the host state directory with `tar` inside the container. Modes and symlinks are
preserved. For non-root `containerUser` profiles, hydrated state is re-owned to
that user so bind-mounted state remains writable after `updateRemoteUserUID`
remaps the user's uid/gid.

Seeding runs before build-preparation hooks (`onCreateCommand`,
`updateContentCommand`, `postCreateCommand`) and is skipped when a profile
declares no state.

`dcc build` records seeded state in `.dcc/<profile>.seed.json`, with per-entry
`seed_digest` and `build_id`. Digests include file bytes and, within directories,
normalized relative paths and symlink targets. Directory traversal order, ownership,
timestamps, and permission modes are intentionally excluded; they do not make identical
seed content appear changed across hosts.

Re-seed behavior:

- If the host state digest matches the recorded `seed_digest`, `dcc` skips
  re-hydration.
- If the host state digest differs, `dcc` preserves your data and warns.
- `dcc build --reseed-state` overwrites differing host state with the image seed.

Runtime commands compare the ledger's `build_id` against the image's `dcc.seed`
label and warn on mismatch. They hydrate only entries with no ledger record, so a
wiped `.dcc` can recover from the image without a rebuild.

`dcc build --dry-run` reports planned seeding without invoking Docker.

## Environment Variables

`dcc` makes a strict distinction between `containerEnv` and `remoteEnv`.

`containerEnv` values are baked into the Docker image as `ENV` directives. They
are available during `docker build` and remain set at runtime. Only the fixed
container-side variables `${containerWorkspaceFolder}` and
`${containerCacheFolder}` may appear in `containerEnv` values.

`remoteEnv` values are passed as `-e KEY=VALUE` flags to `docker run`. They are
not part of the image and are re-evaluated on every run. Host-side variables
such as `${localWorkspaceFolder}`, `${localCacheFolder}`, and `${localEnv:VAR}`
belong in `remoteEnv`, not `containerEnv`.

`${containerEnv:VAR}` is substituted with the value of `VAR` in the built
image's environment, read through `docker image inspect` at runtime. It is valid
in the same places as `${localEnv:VAR}`. The common use is extending a value the
base image set:

```jsonc
"remoteEnv": {
  "PATH": "${containerEnv:PATH}:/opt/tool/bin"
}
```

An absent reference resolves to the empty string unless a fallback is supplied with
`${containerEnv:VAR:default}`. An explicitly present empty `containerEnv` value remains
empty and does not use the fallback; defaults apply only when the key is absent. State
paths are validated after substitution, so an empty result still cannot create a
relative, root, overlapping, or reserved state mount. `${localEnv:VAR}` follows the
same absent/default distinction.

## Commands

The CLI supports these subcommands: `build`, `run`, `exec`, `start`, `attach`,
`stop`, `id`, and `feature`.

### Global Flags

All commands accept `--profile <name>` or `-p <name>`. The default profile is
`devcontainer`.

Global flags include `--strict`, `--dry-run`, `--debug`, and `--format`. They
may appear before or after the subcommand, before any positional command
arguments. For commands such as `dcc exec` and `dcc attach`, whose trailing
arguments form the in-container command, global flags must come before the first
positional command argument or they are passed through to the container command.

Pass `--strict` to treat unrecognised configuration fields as errors instead of
warnings.

Pass `--dry-run` to validate the workspace, profile, config, command line, and
config-local safety gates without invoking Docker. Use `--format json` with
`--dry-run` for a stable machine-readable report.

Structured output outside dry runs is currently supported by
`dcc id --format json` and `dcc feature --format json`.

Pass `--debug` to print resolved command details to stderr without changing
behavior.

### `dcc build`

`dcc build` reads `.devcontainer/<profile>.json` and builds the local Docker
image. Profiles can specify either `image` or official `build` configuration.
The `build` object supports Dockerfile/context builds with `context`,
`dockerfile`, `args`, and `target`. Setting both `image` and `build` is an
error.

`containerUser` defaults to `dev`. When `containerUser` is not `root`, `dcc`
creates the user in the image if needed. When `updateRemoteUserUID` is enabled
and `containerUser` is a non-root named user on Linux or macOS, `dcc` remaps
that user's uid/gid to the host user's uid/gid so bind-mounted workspace,
cache, and state paths are writable. Windows does not perform this remap.

Every `dcc build` stamps the image with a `dcc.version` label and bakes the PID
1 lifecycle supervisor scripts into `/usr/local/share/dcc/`. Runtime commands
refuse to drive an image built by an incompatible `dcc` version. Patch-level
drift is compatible; major/minor drift or a missing label requires a rebuild.

Subsequent builds use Docker's layer cache. Pass `--no-cache` to force a full
rebuild and pass `--pull` to Docker where the build uses an upstream base image,
refreshing moved base image tags. Pass `--refresh-only` to skip image rebuild
and rerun only `updateContentCommand` and `postCreateCommand`; the profile image
must already exist.

Build preparation runs `onCreateCommand`, `updateContentCommand`, and
`postCreateCommand` in order with state mounts attached.

### `dcc feature`

`dcc feature` edits the selected profile's top-level `features` object. See
[Devcontainer Features](features.md#editing-features-with-dcc-feature).

### `dcc run`

`dcc run` runs a named command from `customizations.dcc.commands`. With no
command argument, it lists available commands. Project commands are shown as
`:<name>`. Unqualified command names are accepted only when unique.

Project commands are shell strings:

```jsonc
{
  "customizations": {
    "dcc": {
      "commands": {
        "test": "cargo test",
        "lint": "cargo clippy -- -D warnings"
      }
    }
  }
}
```

Run them by name:

```sh
dcc run test
dcc run :lint
```

Use `dcc exec` for direct command execution. For example,
`dcc run /bin/true` looks for a configured command named `/bin/true`, while
`dcc exec /bin/true` runs `/bin/true` directly.

If the profile container is already running, `dcc run` executes in that
container. If none is running, it starts a one-shot container by default.
One-shot containers stop after all active `dcc`-launched commands finish. Pass
`--keep` or `-k` to keep a newly started container durable or promote an
existing one-shot container.

### `dcc exec`

`dcc exec` runs an explicit command in the profile container:

```sh
dcc exec cargo test
dcc exec npm run build
```

Like `dcc run`, it reuses an existing container when present, otherwise starts a
one-shot container unless `--keep` or `-k` is supplied.

The argument `--` can be supplied to explicitly indicate the boundary between
`dcc` flags and the command. All arguments following `--` are passed through to
the container command.

### `dcc start`

`dcc start` starts the profile container in durable mode without running a
foreground user command. If the container is already running, `dcc start` is
idempotent and promotes it to durable mode.

### `dcc attach`

`dcc attach` attaches to the profile container. If no explicit command is
supplied, `dcc` chooses an interactive shell in this order: executable absolute
`$SHELL`, `/bin/bash`, then `/bin/sh`.

`dcc attach` runs collected `postAttachCommand` hooks host-side before the shell
or explicit attach command. If it starts a new container, it first waits for
`postStartCommand` to finish.

### `dcc stop`

`dcc stop` stops the profile's container if it is running. It is safe to run
when no container is active.

- `dcc stop`: graceful drain. The supervisor stops accepting new commands and
  exits after running commands finish.
- `dcc stop --now`: force-terminates running commands, runs shutdown hooks, then
  exits.
- `dcc stop --kill`: unconditionally kills the container with `docker kill`.

## Lifecycle Hooks

`dcc` separates build preparation from runtime startup. This differs from tools
that treat container creation and editor attach as one flow.

`initializeCommand` is parsed for devcontainer compatibility and produces a
warning, but it is not executed. `dcc` does not run devcontainer-defined commands
on the host.

| Hook | Triggered By | Skipped By | Notes |
| --- | --- | --- | --- |
| `initializeCommand` | None | Always | Parsed and warned as unsupported; never executed. |
| `onCreateCommand` | `dcc build` | `dcc build --refresh-only`, runtime commands, `--dry-run` | Runs in the build-preparation container after the image exists. |
| `updateContentCommand` | `dcc build`, `dcc build --refresh-only` | Runtime commands, `--dry-run` | Runs in the build-preparation container. |
| `postCreateCommand` | `dcc build`, `dcc build --refresh-only` | Runtime commands, `--dry-run` | Runs in the build-preparation container after `updateContentCommand`. |
| `postStartCommand` | `dcc start`, `dcc run`, `dcc exec`, or `dcc attach` only when that invocation starts a new profile container | Reusing an already-running container, `dcc exec --skip-lifecycle`, `--dry-run` | Runs inside the PID 1 supervisor. The host pre-substitutes each hook into a script and bind-mounts the startup hook directory for the supervisor. Foreground commands and attach hooks wait for readiness before proceeding. |
| `postAttachCommand` | `dcc attach` | `dcc run`, `dcc exec`, `dcc start`, `dcc build`, `--dry-run` | Runs host-side immediately before the attach shell or explicit attach command. |

`dcc id` and `dcc stop` do not trigger lifecycle hooks.

For details about hooks contributed by devcontainer Features, see
[Feature lifecycle hooks](features.md#feature-lifecycle-hooks).

Each supported in-container lifecycle hook accepts a shell string, an array of
strings, or an object mapping arbitrary names to either form. Object-form
commands run in parallel, and the next hook waits for all of them to finish.
Hooks run as `containerUser` from `workspaceFolder` and support the same
variable substitution as `remoteEnv` and `mounts`.

To bypass supported runtime hooks for a single `exec` invocation, run:

```sh
dcc exec --skip-lifecycle <command>
```

`dcc` prints a warning naming each skipped script. Build-preparation hooks cannot
be skipped except by using `dcc build --refresh-only`, which skips
`onCreateCommand` only.

## Resource Limits

Runtime container creation defaults to a 4 GiB memory limit and 2 CPUs. These limits
apply to containers created by `dcc run`, `dcc exec`, `dcc attach`, or `dcc start`.
Override either default independently with Docker-equivalent flags:

```sh
dcc run --memory 8g --cpus 6 test
dcc exec --memory 512m npm test
dcc start --memory 8g --cpus 6
```

For example, `dcc start --memory 8g` retains the default 2-CPU limit, while
`dcc start --cpus 6` retains the default 4 GiB memory limit.

## What To Expect Compared With Normal Devcontainers

`dcc` reads standard `.devcontainer/<profile>.json` files, but it is deliberately
more opinionated than a general IDE devcontainer implementation:

- `dcc build` owns `onCreateCommand`, `updateContentCommand`, and
  `postCreateCommand`. Runtime commands do not rerun those hooks.
- Runtime commands use a managed lifecycle supervisor as PID 1 and run
  foreground work through `docker exec`, so `overrideCommand` is ignored.
- `initializeCommand` is parsed and warned about, but not executed.
- `workspaceMount` is ignored. The workspace is always mounted at `/workspace`,
  and `/workspace/.dcc` is masked inside the container.
- `workspaceFolder` controls the workdir for hooks and commands, but it does not
  change where the project is mounted.
- `containerUser` controls the user for hooks and foreground commands. Top-level
  `remoteUser` is not implemented; in strict mode it is an unknown field.
- `forwardPorts` uses a host-side relay into the container's `127.0.0.1`, not
  Docker `-p` publishing. `dcc start` starts the durable container but does not
  leave background port-forwarding processes running.
- `portsAttributes` and `otherPortsAttributes` are parsed for compatibility, but
  browser and preview auto-open behavior is not implemented.
- `runArgs`, sensitive mounts, `privileged`, `capAdd`, and `securityOpt` are
  gated. Host-integrating or privilege-escalating options require
  `--allow-unsafe-runtime`; unknown `runArgs` are rejected.
- `customizations.dcc.state` is the preferred persistence mechanism. It is not an
  arbitrary mount escape hatch.

## Container Identity

Each profile has a stable dcc container id:

```text
dcc-<12hex>--<profile>
```

The `<12hex>` part is the first 12 characters of the SHA-256 hash of a stable
repository identity string. For git repositories with an `origin` remote, this
is the remote URL. For workspaces without a git remote, it falls back to the
canonical workspace root path.

The Docker container name comes from the devcontainer `name` field when present,
falling back to the dcc container id. Invalid Docker container-name characters
are converted to `-` with a warning. Images, caches, and `dcc id` continue to use
the stable dcc container id.

`dcc run` also attaches standard `devcontainer.local_folder` and
`devcontainer.config_file` labels, plus `dcc.container_id`, to every container it
starts.

## Configuration Reference

Unrecognised fields produce a warning by default. Pass `--strict` to treat them
as errors.

### Supported Devcontainer Properties

| Field | Description |
| --- | --- |
| `name` | Human-readable Docker container name. Invalid Docker name characters are converted to `-`; `dcc id`, image tags, and caches still use dcc's stable derived id. |
| `image` | Base Docker image. Mutually exclusive with `build`. |
| `build` | Official Dockerfile/context build source. Supports `context`, `dockerfile`, `args`, and `target`; mutually exclusive with `image`. |
| `features` | devcontainer Features to install. See [Devcontainer Features](features.md). |
| `containerEnv` | Environment variables baked into the Docker image. Supports `${containerWorkspaceFolder}` and `${containerCacheFolder}`. |
| `remoteEnv` | Environment variables passed at runtime. Supports `${localWorkspaceFolder}`, `${localCacheFolder}`, `${localEnv:VAR}`, and `${containerEnv:VAR}`. |
| `containerUser` | User to run hooks and foreground commands as. Defaults to `dev`. |
| `remoteUser` | Not implemented as a top-level field. Use `containerUser`. |
| `updateRemoteUserUID` | Boolean, defaults to `true`. On Linux and macOS, remaps a non-root named `containerUser` to the host uid/gid when safe; Windows is a no-op. |
| `mounts` | Additional bind or volume mounts. Sensitive host sources require `--allow-unsafe-runtime`. |
| `runArgs` | Conservative allowlist of extra Docker runtime flags. Privileged, host-integrating, or unknown flags are gated or rejected. |
| `privileged`, `capAdd`, `securityOpt` | Unsafe runtime settings. Rejected unless the invocation includes `--allow-unsafe-runtime`. |
| `customizations.dcc.extends` | Local config file to inherit from. |
| `customizations.dcc.commands` | Project named shell commands invokable through `dcc run <name>`. |
| `customizations.dcc.state` | Container paths whose contents are persisted under the profile cache. |
| `forwardPorts` | Ports to forward from container to host through the container's loopback interface. |
| `portsAttributes`, `otherPortsAttributes` | Parsed for schema compatibility; browser/preview auto-open behavior is not implemented. |
| `overrideCommand` | Parsed for schema compatibility. Ignored because `dcc` owns PID 1. |
| `workspaceFolder` | Container workdir for hooks and foreground commands. Defaults to `/workspace`. |
| `workspaceMount` | Parsed for schema compatibility, but ignored because `dcc` owns workspace mounting. |
| `initializeCommand` | Parsed for schema compatibility and warned as unsupported. It is not executed. |
| `onCreateCommand`, `updateContentCommand`, `postCreateCommand`, `postStartCommand`, `postAttachCommand` | Supported lifecycle hooks. See [Lifecycle Hooks](#lifecycle-hooks). |
