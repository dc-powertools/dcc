# dcc - Dev Container CLI

`dcc` is a CLI for macOS and Linux that runs devcontainer-based workspaces with
profile-specific images, profile-local state, and predictable container reuse.

It is designed for development workflows where environments are created, reused,
and torn down often. You can keep build artifacts and selected tool state across
containers without sharing state between profiles.

## What It Does

- Builds profile-specific devcontainer images from `.devcontainer/*.json`.
- Runs one-shot commands or durable profile containers.
- Reuses the same running profile container for `run`, `exec`, `attach`, and
  `start`.
- Keeps a durable per-profile `/cache` mount under `.dcc/<profile>`.
- Persists declared state paths with `customizations.dcc.state`.
- Supports local config inheritance with `customizations.dcc.extends`.
- Supports named project commands with `customizations.dcc.commands`.
- Installs devcontainer Features and reads supported Feature metadata.

For the detailed user guide, see [docs/index.md](docs/index.md).

## Platforms

Linux and macOS.

Docker must be installed and running for real build and runtime workflows.

## Installation

```sh
curl -fsSL https://raw.githubusercontent.com/dc-powertools/dcc/main/install.sh | bash
```

The script installs `dcc` to `~/.local/bin/dcc`. Ensure `~/.local/bin` is on
your `PATH`.

## Quick Start

Create a standard devcontainer profile:

```jsonc
// .devcontainer/devcontainer.json
{
  "image": "rust:1",
  "containerUser": "vscode",
  "features": {
    "ghcr.io/devcontainers/features/node:1": {}
  },
  "containerEnv": {
    "CARGO_HOME": "/cache/cargo"
  },
  "customizations": {
    "dcc": {
      "commands": {
        "test": "cargo test",
        "lint": "cargo clippy -- -D warnings"
      },
      "state": [
        "/home/vscode/.cargo",
        { "path": "/home/vscode/.npmrc", "type": "file" }
      ]
    }
  }
}
```

Then build and run the profile:

```sh
dcc build
dcc run test
dcc exec cargo test
dcc attach
dcc stop
```

`dcc build` is explicit: runtime commands do not build the image for you. Run it
after changing the base image, Docker build settings, Features, `containerEnv`,
declared state, forwarded ports, or build-preparation hooks.

## Working With Profiles

The default profile is `.devcontainer/devcontainer.json`. Every command also
accepts `-p/--profile <name>`, which loads `.devcontainer/<name>.json`:

```sh
dcc build -p ci
dcc run -p ci test
```

Each profile gets its own image, container identity, cache, and state directory.
Profile isolation is the default; artifacts are not shared unless you configure
an explicit mount.

## Common Commands

```sh
dcc build                    # build the default profile image
dcc build -p ci              # build .devcontainer/ci.json
dcc build --refresh-only     # rerun update/post-create prep hooks
dcc build --reseed-state     # overwrite declared state with the image seed

dcc run                      # list named project and Feature commands
dcc run test                 # run a named command
dcc exec cargo test          # run an explicit argv directly in the container

dcc feature -a ghcr.io/devcontainers/features/node:1
dcc feature -r ghcr.io/devcontainers/features/node:1

dcc start                    # start or promote a durable profile container
dcc attach                   # run attach hooks, then open a shell
dcc stop                     # stop the profile container with graceful drain
dcc id --format json         # print the stable profile id as JSON
```

All commands accept `--profile <name>` or `-p <name>`. Global flags such as
`--strict`, `--dry-run`, `--debug`, and `--format` may appear before or after the
subcommand, but they must come before any trailing in-container command
arguments.

## Devcontainer Compatibility

`dcc` reads standard `.devcontainer/<profile>.json` files, but it is more
opinionated than a general IDE devcontainer implementation:

- `dcc build` owns build-preparation hooks.
- Runtime commands use a managed PID 1 lifecycle supervisor.
- `initializeCommand` is parsed and warned about, but never executed.
- The workspace is always mounted at `/workspace`.
- `customizations.dcc.state` is the preferred persistence mechanism.
- Host-integrating or privilege-escalating runtime options require
  `--allow-unsafe-runtime`.

See [docs/index.md](docs/index.md#what-to-expect-compared-with-normal-devcontainers)
for the full compatibility notes.

## Documentation

- [User guide](docs/index.md): profiles, cache and state,
  environment variables, commands, lifecycle hooks, configuration reference, and
  devcontainer compatibility.
- [Devcontainer Features](docs/features.md): Feature packages installed through
  the top-level `features` field.
- [Development guide](docs/development.md): local setup, verification, release
  workflow, and maintainer notes.

## Development

For local development and release instructions, see
[docs/development.md](docs/development.md).
