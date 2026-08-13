# dcc — Dev Container CLI

`dcc` is a CLI for macOS and Linux that runs devcontainer-based workspaces with
profile-specific images, profile-local state, and predictable container reuse.

Think of `dcc` as three pieces:

1. `dcc build` prepares the profile image and runs build-preparation lifecycle
   hooks.
2. Runtime commands (`dcc run`, `dcc exec`, `dcc attach`, and `dcc start`) share
   one managed profile container when it is running.
3. State declared under `customizations.dcc.state` is mounted from the
   workspace's `.dcc/<profile>/state` directory, so tool caches and selected files
   can survive one-shot containers and rebuilds.

`dcc` also adds `customizations.dcc.extends` for local config inheritance and
`customizations.dcc.commands` for named project commands. Legacy top-level
`extends` and `scripts` are still accepted with warnings, but new configs should
use the `customizations.dcc` namespace.

`dcc` is designed for the constant churn of environments in agentic coding.
Spinning up and tearing down sessions must be easy, automatic, and safe.


## Platforms

Linux and macOS.


## Installation

```sh
curl -fsSL https://raw.githubusercontent.com/dc-powertools/dcc/main/install.sh | bash
```

The script installs the `dcc` binary to `~/.local/bin/dcc`. Ensure `~/.local/bin` is on your `PATH`.

Requires Docker to be installed and running.


## Working with profiles

`dcc` enables the use of many profile-specific environments called profiles.
The default profile is represented by the standard `devcontainer.json`
configuration. Every command also accepts a `-p/--profile <name>` flag, which
causes `dcc` to load the configuration at `.devcontainer/<name>.json`.

In order to simplify configuration management, `customizations.dcc.extends`
allows inheritance from a common base configuration.

In order to isolate profiles, the durable cache directory described
below is not shared between profiles.


## The `customizations.dcc.extends` property

A devcontainer config file may use `customizations.dcc.extends` to inherit all
properties from another local file. This allows multiple profiles to layer small
changes on top of a common base configuration.

`dcc` generally follows the outline of the proposal in
[devcontainers/spec#22](https://github.com/devcontainers/spec/issues/22).
Arrays and objects are combined as a union of values, while basic types are
overwritten. Lifecycle command fields are not merged; the child value wins.

The path given in "extends" is resolved relative to the file that contains it.
Extension chains (A extends B extends C) are permitted. Circular chains are
invalid and cause `dcc build` to exit with an error.

For example:

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

// Results in
{
    "name": "example/project",
    "forwardPorts": [80, 5432, 2222],  // <-- union
    "containerEnv": {
        "CARGO_HOME": "/cache/cargo",
        "RUST_BACKTRACE": "1"          // <-- overwritten
    },
    "onCreateCommand": "echo hello"
}
```


## The durable cache directory

`dcc` launches devcontainers with a durable cache directory that preserves
artifacts across executions.

Each profile has a unique cache, so artifacts are not shared between profiles.
This makes it easy to spin up and tear down environments, without worrying
about long-running container lifecycles or cross-contamination of environments.

The cache is mounted in the container at `/cache`. The host cache directory is
located within the host workspace directory, under `.dcc/<profile>`.

The cache directory is exposed in devcontainer configuration files through the
following variables:

| Variable | Properties | Description |
| --- | --- | --- |
| `${localCacheFolder}` | Any | Path of the local cache folder. |
| `${containerCacheFolder}` | Any | Path of the cache folder in the container. (`/cache`) |

The preferred way to preserve tool state is to declare the container paths under
`customizations.dcc.state`. String entries are directories; object entries can
declare file state:

```json
"customizations": {
  "dcc": {
    "state": [
      "/home/dev/.cargo",
      { "path": "${containerEnv:HOME}/.npmrc", "type": "file" }
    ]
  }
}
```

Each state path is mounted from `.dcc/<profile>/state/...` on the host. State
paths must be absolute container paths. `${containerWorkspaceFolder}`,
`${containerCacheFolder}`, and `${containerEnv:VAR}` are supported; host-local
variables such as `${localCacheFolder}` and `${localEnv:VAR}` are rejected.
`dcc` also rejects root, relative, overlapping, and unresolved paths.

State bind mounts mask image content with an empty host source, so `dcc` guards
container paths whose masking would break the container or `dcc` itself. These
guards are hard rejects (not gated by `--allow-unsafe-runtime`) and apply both at
config load and after `${containerEnv:VAR}` resolution:

- **Whole subtree blocked** (the path and everything beneath it): `/proc`,
  `/sys`, `/dev`, `/tmp`, `/run`, `/var/run`, `/var/lock`, `/boot`, `/bin`,
  `/sbin`, `/lib`, `/lib32`, `/lib64`, `/libx32`, `/usr/bin`, `/usr/sbin`,
  `/usr/lib`, `/usr/lib32`, `/usr/lib64`, `/usr/libx32`, `/etc`,
  `/workspace/.dcc`, `/cache`, and `/usr/local/share/dcc`.
  `/etc` is blocked as a subtree because empty-file state corrupts `passwd`,
  `group`, and `nsswitch.conf`; use a lifecycle hook to manage system files.
  `/cache` is blocked because it is the profile cache mount itself (self-nesting),
  and `/usr/local/share/dcc` holds `dcc`'s bind-mounted supervisor scripts and hook assets.
- **Exact path only blocked** (specific subdirectories stay valid): `/usr`,
  `/var`, `/home`, `/root`, `/opt`, `/workspace`, `/srv`, `/mnt`, `/media`.
  For example `/usr/local/cargo`, `/var/cache/apt`, `/home/dev/.cargo`, and
  `/workspace/target` remain accepted.

These guards are textual; a state path that is a symlink in the image resolving
outside itself (e.g. `/home/dev/.cache` -> `/etc`) is not detected here.

### Seeding state from the image

Bind mounts never copy image content into an empty host source, so data a
Feature `install.sh`, a `Dockerfile` layer, or an official `build` source places
at a declared state path would be silently masked by an empty directory (or, for
file state, an empty file). `dcc build` therefore **seeds** declared state from
the image: it runs one short-lived container on the finished image with the
state mounts *not* applied and the host state root mounted at `/dcc-seed`, then
copies each declared path's image content into the host state directory with
`tar` (inside the container, so uid, gid, mode, and symlinks are preserved).

Seeding runs **before** build-preparation hooks (`onCreateCommand`,
`updateContentCommand`, `postCreateCommand`), so those hooks observe
install-time content instead of empty directories. It is skipped entirely when a
profile declares no state.

`dcc build` records what it seeded in a host-side ledger at
`.dcc/<profile>.seed.json` (outside the `/cache` mount), with per-entry
`seed_digest` and `build_id`. The ledger is authoritative: an empty seeded
directory and an unseeded one stay distinguishable, and `dcc build` never infers
intent from directory emptiness.

Re-seed policy on `dcc build`:

- If the host state digest matches the recorded `seed_digest`, the state is
  unchanged and `dcc` skips re-hydration (no duplicate work beyond the digest
  check).
- If the host state digest differs, the user has modified it, so `dcc`
  **preserves your data** and warns, naming the path, the recorded seed digest,
  and the `--reseed-state` escape hatch. The image content is *not* overwritten.
- `dcc build --reseed-state` overrides the digest check: differing host state is
  overwritten with the image seed. It is all-or-nothing across every declared
  state path.

`dcc start`, `run`, `exec`, and `attach` compare the ledger's `build_id` against
the image's `dcc.seed` label and warn on mismatch (e.g. a cloned repo with a
stale `.dcc`). They hydrate only entries with no ledger record at all, so a
wiped `.dcc` recovers from the image without a rebuild. Content re-digesting is
deliberately off the runtime hot path.

`dcc build --dry-run` reports planned seeding without invoking Docker.

You can also preserve state within `/cache` by injecting an environment variable
that specifies where to store state. For example:
```
"containerEnv": {
  "CARGO_HOME": "${containerCacheFolder}/.cargo"
}
```

Explicit mounts remain supported when you need the full Docker mount syntax:
```
"mounts": [
  "type=bind,src=${localCacheFolder}/target,dst=/workspace/target"
]
```

Runtime launches automatically create the host-side source directory for any
bind mount whose source path lies under `${localCacheFolder}`, so the directory
does not need to exist before the first use.

The container workspace directory is always `/workspace`.

The `/workspace/.dcc` subdirectory is masked within the container by an
empty tmpfs mount, to prevent data from leaking across profiles.


## `containerEnv` and `remoteEnv`

`dcc` makes a strict distinction between two environment variable properties that the devcontainer spec treats ambiguously.

`containerEnv` values are baked into the Docker image as `ENV` directives. They are available to feature `install.sh` scripts during `docker build` and remain set in the container at runtime. Only the container-side variables `${containerWorkspaceFolder}` and `${containerCacheFolder}` may appear in `containerEnv` values; both resolve to fixed paths (`/workspace` and `/cache`) that are the same on every machine.

`remoteEnv` values are passed as `-e KEY=VALUE` flags to `docker run`. They are not part of the image and are re-evaluated on every run. The host-side variables `${localWorkspaceFolder}` and `${localCacheFolder}` are only valid in `remoteEnv`, because their values are machine-specific absolute paths that would be wrong if baked into an image.

`${containerEnv:VAR}` is substituted with the value of `VAR` in the **built image's** environment — the base image's `ENV` plus every `containerEnv` directive `dcc build` baked in — read via `docker image inspect` at run time. It is valid in the same places as `${localEnv:VAR}` (below). The canonical use is extending a value the base image set, e.g. `"remoteEnv": { "PATH": "${containerEnv:PATH}:/opt/tool/bin" }`. Because the source is the image, it does **not** see `remoteEnv` values (which are not part of the image) or variables set only when the container starts. An undefined reference resolves to the empty string; supply a fallback with `${containerEnv:VAR:default}`. It is not substituted inside `containerEnv` itself.

`${localEnv:VAR}` is substituted with the value of the host environment variable `VAR`, evaluated on every run. It is valid in `remoteEnv`, `mounts`, supported in-container lifecycle hooks, and the container command (the script run by `dcc run` or the arguments to `dcc exec`) — the fields `dcc` resolves at run time. It is **not** substituted in `containerEnv`, which is baked into the image at build time and must not embed host-specific values. An undefined variable resolves to the empty string; supply a fallback with `${localEnv:VAR:default}`.


## Commands

The CLI supports these subcommands: `build`, `run`, `exec`, `start`, `attach`,
`stop`, `id`, and `feature`.

Common workflows:

```sh
dcc build                    # build the default profile image and run build-prep hooks
dcc build -p ci              # build .devcontainer/ci.json
dcc build --refresh-only     # rerun update/post-create prep hooks; image must exist
dcc build --reseed-state     # overwrite modified declared state with the image seed

dcc run                      # list named project and Feature commands
dcc run test                 # run a named command from customizations.dcc.commands
dcc exec cargo test          # run an explicit argv directly in the container
dcc feature -a ghcr.io/devcontainers/features/node:1
dcc feature -r ghcr.io/devcontainers/features/node:1

dcc start                    # start or promote a durable profile container
dcc attach                   # run attach hooks, then open a shell
dcc stop                     # stop the profile container (graceful drain)
dcc id --format json         # print the stable profile id as JSON
```

`dcc build` is explicit: runtime commands do not build the image for you. Run it
after changing `image`, `build`, Features, `containerEnv`, forwarded ports,
declared state, or build-preparation hooks.

### Global flags

All commands accept `--profile <name>` or `-p <name>`. The default profile is
`devcontainer`, which loads `.devcontainer/devcontainer.json`; `-p ci` loads
`.devcontainer/ci.json`.

Because `--profile`, `--strict`, `--dry-run`, `--debug`, and `--format` are
global flags, they may appear before or after the subcommand, before any
positional command arguments. For commands like `dcc exec` and `dcc attach`,
whose trailing arguments form the in-container command, global flags must come
before the first positional command argument or they are passed through to the
container command.

Pass `--strict` to treat unrecognised configuration fields as errors instead of
warnings.

Pass `--dry-run` to validate the workspace, profile, config, command line, and
config-local safety gates without invoking Docker. Dry runs stop before Docker
image inspection, image build/pull/tag, container lookup/start/exec/stop, port
forwarding, and lifecycle hook execution.

By default a dry run prints a short text report and exits 0 when validation
succeeds. Pass `--format json` with `--dry-run` for a stable machine-readable
report containing the command, profile, config path, `docker_invoked: false`,
checks performed, and Docker-dependent checks skipped.

Outside dry runs, structured output is currently supported by `dcc id --format
json`, which prints the resolved profile container id, and `dcc feature --format
json`, which prints the profile feature edit summary.

Dry runs cannot validate information that only exists in Docker image metadata,
such as Feature-contributed command metadata from `devcontainer.metadata`, or
values that require inspecting/probing the built image. The live Docker smoke
tests remain the source of truth for build/run behavior past that boundary.

Pass `--debug` to print resolved command details to stderr without changing
behavior. Runtime commands print launch details just before the container starts:
the container name and image, the runtime environment (`remoteEnv`) and
image-baked `containerEnv`, every mount with its resolved `src -> dst` and
options, forwarded ports, lifecycle scripts in execution order, and the exact
`docker run` command. `dcc build`, `dcc stop`, and `dcc id` print the resolved
profile, config, image, and container identity details available to those
commands.

### `dcc build`

Reads `.devcontainer/<profile>.json` and builds the local Docker image.

Profiles can specify either `image` or official `build` configuration. `build`
supports Dockerfile/context builds with `context`, `dockerfile`, `args`, and
`target`. Setting both `image` and `build` is an error.

`containerUser` defaults to `dev` when not set. When the profile uses only
`image`, has `containerUser` set to `root`, and has no Features, containerEnv,
forwarded ports, state, or build-preparation hooks, `dcc` takes a fast path: it
pulls the base image and retags it locally without a Dockerfile build.

Otherwise, `dcc` generates a Dockerfile. When `containerUser` is not `root`,
`dcc` adds a `RUN` step to the Dockerfile that creates the user if it does not
already exist; this step is cross-distro compatible (`useradd` for
Debian/Ubuntu/RHEL, `adduser` for Alpine). When `features` are also set, the
user is created first. Immediately after user creation, when
`updateRemoteUserUID` is enabled (the default) and `containerUser` is a non-root
named user on a Linux host, `dcc` adds a remap `RUN` that rewrites the user's
uid/gid in `/etc/passwd` and `/etc/group` to the host user's uid/gid and
`chown`s the user's home folder, so bind-mounted host content (workspace, cache,
state) is writable regardless of the host user's uid. The remap is skipped for
`root` and numeric users, when the uid/gid already match, when another user
already occupies the target uid, and on non-Linux hosts.

Each feature's `install.sh` runs as `root`, matching the containers.dev
feature spec that most published features assume (e.g. for `apt-get`).
`dcc` exports `_REMOTE_USER`, `_CONTAINER_USER`, `_REMOTE_USER_HOME`, and
`_CONTAINER_USER_HOME` so a script can `su "$_REMOTE_USER" -c '...'` for any
steps that need to run as `containerUser` (e.g. dotfiles, per-user tool
installs).

Subsequent builds are incremental via Docker's layer cache; pass `--no-cache`
to force a full rebuild. Pass `--refresh-only` to skip image rebuild and rerun
only `updateContentCommand` and `postCreateCommand`; it fails if the profile
image does not already exist.

The generated Dockerfile stamps the installed `dcc` version as a `LABEL`
immediately after `FROM`, so upgrading `dcc` automatically invalidates the cache
for every dcc-controlled step. `dcc` also installs build-preparation hook assets
into the image. The PID 1 lifecycle supervisor scripts are not baked into the
image — they are bind-mounted read-only from the host at runtime, so they exist
in every container including the fast path. During
`dcc build`, build preparation runs `onCreateCommand`, `updateContentCommand`,
and `postCreateCommand` in order, with Feature hooks before project hooks for
each phase and state mounts attached.

### `dcc feature`

Adds or removes entries in the selected profile's top-level `features` object.
New features are added with empty options (`{}`), and existing options are left
unchanged when a feature is already present:

```sh
dcc feature --add ghcr.io/devcontainers/features/node:1
dcc feature -a ghcr.io/devcontainers/features/python:1
dcc feature --remove ghcr.io/devcontainers/features/node:1
dcc feature -r ghcr.io/devcontainers/features/python:1
```

`--add`/`-a` and `--remove`/`-r` may be repeated in one invocation. Removals are
applied before additions. The command edits only the selected profile file
(`.devcontainer/devcontainer.json` by default, or `.devcontainer/<profile>.json`
with `--profile`). It does not edit parent configs referenced through
`customizations.dcc.extends`, and it does not build the image; run `dcc build`
after changing Features.

The command parses JSONC input but rewrites the profile file as formatted JSON.
Use `--dry-run` to validate and preview the operation without writing.

### `dcc run`

Runs a named command from `customizations.dcc.commands` or Feature command
metadata. With no command argument, `dcc run` lists available commands. Project
commands are shown as `:<name>`; Feature commands are shown as
`<feature-id>:<name>`. Unqualified command names are accepted only when unique.

Project commands are shell strings:

```json
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

Then run them by name:

```sh
dcc run test
dcc run :lint
```

`dcc run` does not treat the argument as a host path or arbitrary executable.
For example, `dcc run /bin/true` looks for a configured command named
`/bin/true`; use `dcc exec /bin/true` for direct execution.

If the profile container is already running, `dcc run` executes the command in
that container. If none is running, it starts a one-shot container by default.
One-shot containers stop after all active `dcc`-launched commands finish, so
concurrent `dcc run` and `dcc exec` invocations can share the same temporary
container. Pass `--keep` or `-k` to keep a newly started container durable, or to
promote an existing one-shot container.

`dcc build` must be run before runtime commands; `dcc run` never builds the image
automatically.

### `dcc exec`

Runs an explicit command in the profile container:

```sh
dcc exec cargo test
dcc exec npm run build
```

Like `dcc run`, it reuses an existing container when present, otherwise starts a
one-shot container unless `--keep` / `-k` is supplied.

The argument `--` can be supplied to explicitly indicate the boundary between
`dcc` flags and the command. All arguments following `--` are passed through to
the container command.

### `dcc start`

Starts the profile container in durable mode without running a foreground user
command. If the container is already running, `dcc start` is idempotent and
promotes it to durable mode.

### `dcc attach`

Attaches to the profile container. If no explicit command is supplied, `dcc`
chooses an interactive shell in this order: executable absolute `$SHELL`,
`/bin/bash`, then `/bin/sh`. `dcc attach` runs collected `postAttachCommand`
hooks before the shell or explicit attach command. `dcc run` and `dcc exec` do
not run attach hooks by default.

### `dcc stop`

Stops the profile's container if it is running. It is safe to run when no container
is active (idempotent).

- **`dcc stop`** (default): signals the in-container supervisor to stop accepting new
  commands and exit after all running commands finish (graceful drain).
- **`dcc stop --now`**: force-terminates running commands, runs shutdown hooks, then
  exits.
- **`dcc stop --kill`**: unconditionally kills the container (`docker kill`). Use this
  when the container is wedged or corrupted and the supervisor is unresponsive.


## Lifecycle hooks

`dcc` separates build preparation from runtime startup. This is different from
tools that treat container creation and editor attach as one flow.

`initializeCommand` is parsed for devcontainer compatibility and produces a
warning, but it is not executed. `dcc` does not run devcontainer-defined commands
on the host.

| Hook | Triggered by | Skipped by | Notes |
|---|---|---|---|
| `initializeCommand` | None | Always | Parsed and warned as unsupported; never executed. |
| `onCreateCommand` | `dcc build` | `dcc build --refresh-only`, all runtime commands, `--dry-run` | Runs in the build-preparation container after the image exists. |
| `updateContentCommand` | `dcc build`, `dcc build --refresh-only` | Runtime commands, `--dry-run` | Runs in the build-preparation container. |
| `postCreateCommand` | `dcc build`, `dcc build --refresh-only` | Runtime commands, `--dry-run` | Runs in the build-preparation container after `updateContentCommand`. |
| `postStartCommand` | `dcc start`, `dcc run`, `dcc exec`, or `dcc attach` only when that invocation starts a new profile container | Reusing an already-running container, `dcc exec --skip-lifecycle`, `--dry-run` | Runs in the runtime container before the foreground command or attach shell. |
| `postAttachCommand` | `dcc attach` | `dcc run`, `dcc exec`, `dcc start`, `dcc build`, `--dry-run` | Runs immediately before the attach shell or explicit attach command. If `dcc attach` starts a new container, `postStartCommand` runs first. |

`dcc id` and `dcc stop` do not trigger lifecycle hooks.

For in-container hooks, Feature-contributed hooks of that type run first, in
Feature installation order, followed by the `devcontainer.json` hook of that
type. A non-zero exit from any hook aborts the current operation and skips
subsequent hooks.

Each supported in-container lifecycle hook accepts a shell string (run via
`/bin/sh -c`), an array of strings (executed directly), or an object mapping
arbitrary names to either form. Object-form commands run in parallel, and the
next hook waits for all of them to finish. Hooks run as `containerUser` from
`workspaceFolder` and support the same variable substitution as
`remoteEnv`/`mounts`.

To bypass supported runtime hooks for a single `exec` invocation, run
`dcc exec --skip-lifecycle <command>`. `dcc` prints a warning naming each skipped
script, so nothing is silently omitted. Build-preparation hooks cannot be skipped
except by using `dcc build --refresh-only`, which skips `onCreateCommand` only.


## What to expect compared with normal devcontainers

`dcc` reads standard `.devcontainer/<profile>.json` files, but it is deliberately
more opinionated than a general IDE devcontainer implementation:

- `dcc build` owns `onCreateCommand`, `updateContentCommand`, and
  `postCreateCommand`. Runtime commands do not rerun those hooks.
- Runtime commands use a managed lifecycle supervisor as PID 1 and run foreground
  work through `docker exec`, so `overrideCommand` is ignored.
- `initializeCommand` is parsed and warned about, but not executed. `dcc` avoids
  running devcontainer-defined commands on the host.
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
- `runArgs`, sensitive mounts, `privileged`, `capAdd`, `securityOpt`, and unsafe
  Feature runtime settings are gated. Host-integrating or privilege-escalating
  options require `--allow-unsafe-runtime`; unknown `runArgs` are rejected.
- `customizations.dcc.state` is the preferred persistence mechanism. It is not an
  arbitrary mount escape hatch; paths are validated and stored under the
  profile-local `.dcc/<profile>/state` directory.


## Configuration

`dcc` searches for the `.devcontainer` directory by walking up from the current
working directory through its ancestors, stopping at the first directory that
contains a `.devcontainer` directory.

This means you can run `dcc` from any subdirectory of a project.

`dcc` does not support standalone `.devcontainer.json` files. All profile
configurations must be located within the `.devcontainer` directory.


### Container identity

Each profile has a stable dcc container id in the form:

```
dcc-<12hex>--<profile>
```

The `<12hex>` part is the first 12 characters of the SHA-256 hash of a stable
**repository identity string**. For git repositories with an `origin` remote,
this is the remote URL (e.g. `https://github.com/org/repo`). For workspaces
without a git remote, it falls back to the canonical workspace root path.

Using the remote URL means the container id is the same on every machine that
clones the same repository, regardless of where the directory is located. Renaming
or moving the directory does not change the container id.

The Docker container name comes from the devcontainer `name` field when present,
falling back to the dcc container id. If `name` contains characters Docker does
not accept in container names, `dcc` converts them to `-` and prints a warning.
Images, caches, and `dcc id` continue to use the stable dcc container id.

`dcc run` also attaches the standard `devcontainer.local_folder` and
`devcontainer.config_file` labels, plus `dcc.container_id`, to every container it
starts, making dcc containers discoverable by VS Code and other
devcontainer-compatible tools via
`docker ps --filter label=devcontainer.local_folder=<path>`.

### Supported devcontainer configuration properties

| Field | Description |
|---|---|
| `name` | Human-readable Docker container name. Invalid Docker name characters are converted to `-`; `dcc id`, image tags, and caches still use dcc's stable derived id. |
| `image` | Base Docker image |
| `build` | Official Dockerfile/context build source. Supports `context`, `dockerfile`, `args`, and `target`; mutually exclusive with `image`. |
| `features` | devcontainer Features to install |
| `containerEnv` | Environment variables baked into the Docker image as `ENV` directives. Supports `${containerWorkspaceFolder}` and `${containerCacheFolder}`. |
| `remoteEnv` | Environment variables passed as runtime flags to `docker run`. Supports `${localWorkspaceFolder}`, `${localCacheFolder}`, `${localEnv:VAR}`, and `${containerEnv:VAR}`. |
| `containerUser` | User to run as inside the container. Defaults to `dev`. Unless set to `root`, `dcc build` creates the user in the image if it does not already exist. Feature install scripts run as `root`; `_REMOTE_USER`/`_CONTAINER_USER`/`_REMOTE_USER_HOME`/`_CONTAINER_USER_HOME` are exported for scripts that need to `su` into `containerUser`. |
| `remoteUser` | Not implemented as a top-level field. Default mode warns because it is unrecognised; `--strict` rejects it. Use `containerUser` for the user that runs hooks and foreground commands. |
| `updateRemoteUserUID` | Boolean, defaults to `true`. On Linux, when `containerUser` is a non-root named user, `dcc build` remaps that user's uid/gid to the host user's uid/gid inside the image so bind mounts (workspace, cache, state) are writable regardless of the host user's uid. The remap safely no-ops when the user is `root` or numeric, the uid/gid already match, another user already occupies the target uid, or the host is not Linux. Set `false` to disable. |
| `mounts` | Additional bind or volume mounts. Supports `${localWorkspaceFolder}`, `${localCacheFolder}`, `${localEnv:VAR}`, and `${containerEnv:VAR}`. Sensitive host sources such as `/`, `/etc`, `/var/run`, Docker sockets, and SSH paths require `--allow-unsafe-runtime`. |
| `runArgs` | Conservative allowlist of extra Docker runtime flags. Safe flags such as `--add-host`, `--dns`, `--hostname`, `--label`, `--tmpfs`, `--shm-size`, `--ulimit`, `--platform`, `--cap-drop`, and explicit `--env KEY=VALUE` are passed through. Privileged or host-integrating flags such as `--privileged`, `--cap-add`, `--security-opt`, `--pid=host`, `--ipc=host`, `--network=host`, `--device`, and sensitive mounts require `--allow-unsafe-runtime`; unknown flags are rejected. |
| `privileged`, `capAdd`, `securityOpt` | Unsafe runtime settings. Rejected unless the current `dcc build`, `dcc start`, `dcc run`, `dcc exec`, or `dcc attach` invocation includes `--allow-unsafe-runtime`. |
| `customizations.dcc.extends` | Local config file to inherit from. Parent arrays and objects are merged, and child scalar values win. Legacy top-level `extends` still works with a warning. |
| `customizations.dcc.commands` | Project named shell commands invokable through `dcc run <name>`. Legacy top-level `scripts` still works with a warning. |
| `customizations.dcc.state` | Container paths whose contents are persisted under the profile cache and mounted back into the container. String entries are directories; object entries support `{ "path": "...", "type": "file" }`. |
| `forwardPorts` | Ports to forward from container to host. Each port is tunnelled through the container's loopback interface so the application sees connections as coming from `127.0.0.1`. `dcc build` installs `nc` (netcat) in the image automatically to enable this. |
| `portsAttributes`, `otherPortsAttributes` | Parsed for schema compatibility. `label`, `protocol`, and `onAutoForward` values `openBrowser`, `openBrowserOnce`, `openPreview`, `silent`, and `ignore` are accepted; browser/preview auto-open behavior is not implemented. |
| `overrideCommand` | Parsed for schema compatibility. Ignored because `dcc` always uses its managed lifecycle supervisor as PID 1. |
| `workspaceFolder` | Container workdir for build-preparation hooks, startup/attach hooks, and foreground commands. Defaults to `/workspace`; `dcc` warns when it is outside `/workspace` while still mounting the project at `/workspace`. |
| `workspaceMount` | Parsed for schema compatibility, but ignored because `dcc` owns workspace mounting. |
| `initializeCommand` | Parsed for schema compatibility and warned as unsupported. It is not executed. |
| `onCreateCommand`, `updateContentCommand`, `postCreateCommand`, `postStartCommand`, `postAttachCommand` | Supported in-container lifecycle hooks. See [Lifecycle hooks](#lifecycle-hooks) for trigger behavior, ordering, supported forms, and skip behavior. |

Unrecognised fields produce a warning by default; pass `--strict` to treat them as errors.

### Supported feature properties (`devcontainer-feature.json`)

The following properties in a feature's `devcontainer-feature.json` are read and acted upon by `dcc`.

| Property | Description |
|---|---|
| `options` | Configuration options. Keys are uppercased and passed as environment variables to `install.sh`. User-supplied values override declared defaults. |
| `containerEnv` | Environment variables baked into the image as Dockerfile `ENV` directives, set before the feature's `install.sh` runs. |
| `remoteEnv` | Environment variables passed as runtime flags to `docker run`. Stored as templates; `${localWorkspaceFolder}`, `${localCacheFolder}`, `${localEnv:VAR}`, and `${containerEnv:VAR}` are substituted at run time. |
| `mounts` | Additional mounts attached when the runtime profile container starts. Each entry is a JSON object with `type`, `source`, and `target` fields — the same format accepted by Docker's `--mount` flag. Supports the same variable substitution as `devcontainer.json` mounts (`${localCacheFolder}`, `${localEnv:VAR}`, `${containerEnv:VAR}`, etc.). |
| `customizations.dcc.commands` | Named shell commands invokable through `dcc run`. Feature commands are addressed as `<feature-id>:<command>`; legacy top-level `scripts` still works with a deprecation warning. |
| `customizations.dcc.state` | Feature-contributed persistent state paths. Uses the same validation and cache mount behavior as project `customizations.dcc.state`; Feature state is mounted before project state. |
| `installsAfter` | Soft ordering hint. An array of feature IDs (the `id` field from `devcontainer-feature.json`). This feature is installed after the listed features if they are already in the installation set. Not evaluated recursively. |
| `dependsOn` | Hard dependencies. An object whose keys are feature references (same format as `devcontainer.json` `features`) and values are the options for each dependency. Missing dependencies are added to the installation set automatically. Evaluated recursively. Circular dependencies are an error. |
| `onCreateCommand`, `updateContentCommand`, `postCreateCommand`, `postStartCommand`, `postAttachCommand` | Lifecycle hooks. Same forms and variable substitution as the identically-named `devcontainer.json` properties. For each hook type, feature-contributed hooks run before the `devcontainer.json` hook of that type, in feature installation order. |
| `init`, `entrypoint` | Parsed for compatibility but ignored with a warning because `dcc` owns PID 1 (the lifecycle supervisor). |
| `privileged`, `capAdd`, `securityOpt` | Unsafe runtime settings. Rejected unless the current `dcc build`, `dcc start`, `dcc run`, `dcc exec`, or `dcc attach` invocation includes `--allow-unsafe-runtime`. |

Feature `containerUser` and `remoteUser` are rejected because the Feature schema does
not permit Features to set users.

### Example

```json
{
  "image": "rust:1",
  "features": {
    "ghcr.io/devcontainers/features/node:1": {}
  },
  "containerEnv": {
    "RUST_BACKTRACE": "1"
  },
  "containerUser": "vscode"
}
```

## Resource Limits

Runtime container creation defaults to **4 GB memory** and **2 CPUs**. Override
with Docker-equivalent flags on `dcc run`, `dcc exec`, `dcc attach`, or
`dcc start`:

```sh
dcc run --memory 8g --cpus 6 test
dcc exec --memory 512m npm test
dcc start --memory 8g --cpus 6
```

## Releasing

To cut a release, bump the version and push to `main`:

```sh
scripts/bump.sh patch     # or: minor | major
git push origin main
```

`scripts/bump.sh` edits the version in `Cargo.toml`, refreshes `Cargo.lock`, and
commits `chore: bump version to vX.Y.Z`. When the push lands on `main`, the
**Auto-tag on version change** workflow (`.github/workflows/autotag.yml`) runs CI
(format, clippy, tests, build) and, **only if it passes**, creates the matching
`vX.Y.Z` tag if it does not already exist, which triggers the **Release** workflow
to build the four target binaries and publish a GitHub Release. If CI fails, no tag
or release is produced. A push that changes `Cargo.toml` without changing the
version is a no-op (the tag already exists).

Alternatively, run the **Bump Version** workflow from the Actions tab (choose
`patch`/`minor`/`major`); it performs the same steps in CI via `scripts/bump.sh`
and the auto-tag workflow.
