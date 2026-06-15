# dcc — Dev Container CLI

`dcc` is a CLI for macOS and Linux that streamlines the use of devcontainers.

`dcc` facilitates the use of multiple ephemeral devcontainers with different
profiles across the development cycle. It introduces two extensions to the
devcontainer spec that make this possible:

1. a durable cache directory, which persists artifacts across executions
2. the "extends" property, which enables inheritance of common configuration

`dcc` is designed for the constant churn of environments in agentic coding.
Spinning up and tearing down sessions must be easy, automatic, and safe.


## Working with profiles

`dcc` enables the use of many ephemeral environments called profiles.
The default profile is represented by the standard `devcontainer.json`
configuration. Every command also accepts a `-p/--profile <name>` flag, which
causes `dcc` to load the configuration at `.devcontainer/<name>.json`.

In order to simplify configuration management, the "extends" property
described below allows inheritance from a common base configuration.

In order to isolate profiles, the durable cache directory described
below is not shared between profiles.


## The "extends" property

A devcontainer config file may use the property "extends" to indicate that
it inherits all properties from another local file. This allows multiple
profiles to layer small changes on top of a common base configuration.

`dcc` generally follows the outline of the proposal in
[devcontainers/spec#22](https://github.com/devcontainers/spec/issues/22).
Arrays and objects are combined as a union of values, while basic types are
overwritten. Exception: `command` always takes the child value (see below).

The path given in "extends" is resolved relative to the file that contains it.
Extension chains (A extends B extends C) are permitted. Circular chains are
invalid and cause `dcc build` to exit with an error.

For example:

```
// .devcontainer/base.json
{
    "name": "example/project",
    "forwardPorts": [80, 5432],
    "hostRequirements": {
        "storage": "64gb",
        "memory": "16gb"
    }
}

// .devcontainer/derived.json
{
    "extends": "./base.json",
    "forwardPorts": [80, 2222],
    "hostRequirements": {
        "memory": "32gb"
    },
   "onCreateCommand": "echo hello"
}

// Results in
{
    "name": "example/project",
    "forwardPorts": [80, 5432, 2222],  // <-- union
    "hostRequirements": {
        "storage": "64gb",
        "memory": "32gb"               // <-- overwritten
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

The cache directory is exposed in devcontainer configuration files through
the following variables:

| Variable | Properties | Description |
| --- | --- | --- |
| `${localCacheFolder}` | Any | Path of the local cache folder. |
| `${containerCacheFolder}` | Any | Path of the cache folder in the container. (`/cache`) |

There are two primary ways to preserve state within the cache. The first is to
inject an environment variable that specifies where to store state. For example:
```
"containerEnv": {
  "CARGO_HOME": "${containerCacheFolder}/.cargo"
}
```

The second is to mount a cache subdirectory at the location where state is
stored. For example:
```
"mounts": [
  "type=bind,src=${localCacheFolder}/target,dst=/workspace/target"
]
```

`dcc run` automatically creates the host-side source directory for any bind
mount whose source path lies under `${localCacheFolder}`, so the directory
does not need to exist before the first run.

The container workspace directory is always `/workspace`.

The `/workspace/.dcc` subdirectory is masked within the container by an
empty tmpfs mount, to prevent data from leaking across profiles.


## `containerEnv` and `remoteEnv`

`dcc` makes a strict distinction between two environment variable properties that the devcontainer spec treats ambiguously.

`containerEnv` values are baked into the Docker image as `ENV` directives. They are available to feature `install.sh` scripts during `docker build` and remain set in the container at runtime. Only the container-side variables `${containerWorkspaceFolder}` and `${containerCacheFolder}` may appear in `containerEnv` values; both resolve to fixed paths (`/workspace` and `/cache`) that are the same on every machine.

`remoteEnv` values are passed as `-e KEY=VALUE` flags to `docker run`. They are not part of the image and are re-evaluated on every run. The host-side variables `${localWorkspaceFolder}` and `${localCacheFolder}` are only valid in `remoteEnv`, because their values are machine-specific absolute paths that would be wrong if baked into an image.

`dcc` does not support `${containerEnv:VAR}` references in `remoteEnv`. This diverges from the devcontainer specification, which permits `remoteEnv` values to reference variables from the live container environment.


## Commands

All commands accept the flag `--profile <name>` that indicates which profile to load.
The default profile is simply `devcontainer`, which loads from the standard config
file location `.devcontainer/devcontainer.json`.

### `dcc build`

Reads `.devcontainer/<profile>.json` and builds the local Docker image.

When neither `features` nor `containerUser` are set, `dcc` takes a fast path:
it pulls the base image and retags it locally without a Dockerfile build.

When `features` are set, `dcc` generates a Dockerfile that installs them on top
of the base image. When `containerUser` is set, `dcc` adds a `RUN` step to the
Dockerfile that creates the user if it does not already exist; this step is
cross-distro compatible (`useradd` for Debian/Ubuntu/RHEL, `adduser` for
Alpine). Both conditions may apply simultaneously: the user is created first,
and each feature's `install.sh` is then run as that user rather than root.

Subsequent builds are incremental via Docker's layer cache; pass `--no-cache`
to force a full rebuild.

### `dcc run`

Starts the profile's container and runs its configured `command`. Attaches
an interactive terminal and pipes stdin/stdout until the container exits.
Containers are always ephemeral. `dcc run` terminates with an error if the
profile's container is already running. `dcc build` must be run before `dcc run`;
`dcc run` never builds the image automatically.

If additional arguments are present, such as `dcc run npm serve`, they override
the configured command. Starting from the first non-flag argument,
all subsequent arguments are passed through as the launch command.

The argument `--` can be supplied to explicitly indicate the boundary between
`dcc` flags and the container launch command. All arguments following `--` will
be passed through to the container.

### `dcc join`

Reattaches to the original process's stdin/stdout after detaching from a running
container via Docker's escape key sequence. This should not normally be required.

### `dcc stop`

Stops the profile's container if it is running. This should not normally be
required.


## Configuration

`dcc` searches for the `.devcontainer` directory by walking up from the current
working directory through its ancestors, stopping at the first directory that
contains a `.devcontainer` directory.

This means you can run `dcc` from any subdirectory of a project.

`dcc` does not support standalone `.devcontainer.json` files. All profile
configurations must be located within the `.devcontainer` directory.


### Container identity

Each profile's container is identified by a name derived from the local
workspace directory and the profile name:

```
<workspace-basename>--<profile>
```

For example, a project in `~/code/my-project` with the `claude` profile uses
the container name `my-project--claude`.

### Supported devcontainer configuration properties

| Field | Description |
|---|---|
| `image` | Base Docker image |
| `features` | devcontainer Features to install |
| `containerEnv` | Environment variables baked into the Docker image as `ENV` directives. Supports `${containerWorkspaceFolder}` and `${containerCacheFolder}`. |
| `remoteEnv` | Environment variables passed as runtime flags to `docker run`. Supports `${localWorkspaceFolder}` and `${localCacheFolder}`. |
| `containerUser` | User to run as inside the container. When set, `dcc build` creates the user in the image if it does not already exist. When absent, Docker uses the image's `USER` directive. |
| `mounts` | Additional bind or volume mounts |
| `forwardPorts` | Ports to forward from container to host. Each port is tunnelled through the container's loopback interface so the application sees connections as coming from `127.0.0.1`. `dcc build` installs `nc` (netcat) in the image automatically to enable this. |
| `command` | Array of strings passed to Docker as `--entrypoint` when the container starts. The child value always takes precedence over the parent when using `extends`. Always wins over any feature-contributed command. |

Unrecognised fields produce a warning by default; pass `--strict` to treat them as errors.

### Supported feature properties (`devcontainer-feature.json`)

The following properties in a feature's `devcontainer-feature.json` are read and acted upon by `dcc`.

| Property | Description |
|---|---|
| `options` | Configuration options. Keys are uppercased and passed as environment variables to `install.sh`. User-supplied values override declared defaults. |
| `command` | Array of strings passed to Docker as `--entrypoint` when the container starts. The last feature in installation order wins; if multiple features declare a command a warning is emitted. The top-level `command` in `devcontainer.json` always overrides feature-contributed commands (with a warning). |
| `containerEnv` | Environment variables baked into the image as Dockerfile `ENV` directives, set before the feature's `install.sh` runs. |
| `remoteEnv` | Environment variables passed as runtime flags to `docker run`. Stored as templates; `${localWorkspaceFolder}` and `${localCacheFolder}` are substituted at run time. |
| `mounts` | Additional mounts attached at `dcc run` time. Each entry is a JSON object with `type`, `source`, and `target` fields — the same format accepted by Docker's `--mount` flag. Supports the same variable substitution as `devcontainer.json` mounts (`${localCacheFolder}`, etc.). |
| `installsAfter` | Soft ordering hint. An array of feature IDs (the `id` field from `devcontainer-feature.json`). This feature is installed after the listed features if they are already in the installation set. Not evaluated recursively. |
| `dependsOn` | Hard dependencies. An object whose keys are feature references (same format as `devcontainer.json` `features`) and values are the options for each dependency. Missing dependencies are added to the installation set automatically. Evaluated recursively. Circular dependencies are an error. |

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

`dcc run` defaults to **4 GB memory** and **4 CPUs**. Override with Docker-equivalent flags:

```sh
dcc run --memory 8g --cpus 6
dcc run --memory 512m npm test
```

## Installation

```sh
curl -fsSL https://raw.githubusercontent.com/dc-powertools/dcc/main/install.sh | bash
```

The script installs the `dcc` binary to `~/.local/bin/dcc`. Ensure `~/.local/bin` is on your `PATH`.

Requires Docker to be installed and running.

## Platforms

Linux and macOS.
