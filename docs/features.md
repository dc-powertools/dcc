# Devcontainer Features

Devcontainer Features are reusable packages that add tools, runtimes, or setup
steps to a development container. They are declared in a devcontainer config's
top-level `features` field and usually include an `install.sh` script plus
Feature metadata. See the official
[Dev Container Features reference](https://containers.dev/implementors/features/)
for the underlying format.

This page covers how `dcc` installs and uses those Feature packages. For the
general `dcc` usage guide, see [docs/index.md](index.md).

## Adding Features To A Profile

Declare Features in `.devcontainer/<profile>.json` under the top-level
`features` object:

```jsonc
{
  "image": "rust:1",
  "features": {
    "ghcr.io/devcontainers/features/node:1": {},
    "ghcr.io/devcontainers/features/python:1": {
      "version": "3.12"
    }
  }
}
```

The object key is the Feature reference. The value is the options object passed
to that Feature. Run `dcc build` after adding, removing, or changing Features.

## Editing Features With `dcc feature`

`dcc feature` adds or removes entries in the selected profile's top-level
`features` object. New Features are added with empty options (`{}`), and
existing options are left unchanged when a Feature is already present:

```sh
dcc feature --add ghcr.io/devcontainers/features/node:1
dcc feature -a ghcr.io/devcontainers/features/python:1
dcc feature --remove ghcr.io/devcontainers/features/node:1
dcc feature -r ghcr.io/devcontainers/features/python:1
```

`--add`/`-a` and `--remove`/`-r` may be repeated in one invocation. Removals are
applied before additions. The command edits only the selected profile file and
does not edit parent configs referenced through `customizations.dcc.extends`.

The command parses JSONC input but rewrites the profile file as formatted JSON.
Use `--dry-run` to validate and preview the operation without writing. Use
`--format json` for structured edit summaries.

## Build Behavior

Each Feature's `install.sh` runs during `dcc build` as `root`, matching the
containers.dev Feature convention. `dcc` exports `_REMOTE_USER`,
`_CONTAINER_USER`, `_REMOTE_USER_HOME`, and `_CONTAINER_USER_HOME` so Feature
scripts can switch to `containerUser` for per-user setup.

Feature option values come from the selected profile's `features` object. Option
keys are uppercased and passed to `install.sh` as environment variables; values
from the profile override defaults declared by the Feature.

For compatibility, a Feature containing only `install.sh` and no
`devcontainer-feature.json` is accepted as an install-only Feature: explicit profile
options are still passed through, but there are no metadata defaults or runtime
contributions. When metadata is present, it must be valid JSON with valid field types;
malformed supplied metadata is a build error.

`dcc` preserves the declaration order of independent Features. `dependsOn` adds
missing hard dependencies recursively, while `installsAfter` acts as a soft
ordering hint for Features that are already in the installation set. Circular
dependencies are an error.

Feature references resolve from upstream during `dcc build`. Passing
`dcc build --no-cache` also passes `docker build --pull` where the build uses an
upstream base image, so Docker refreshes moved base image tags instead of reusing
a stale local base.

### Private registries and custom CAs

Profiles can trust a private CA for one exact OCI authority through
`customizations.dcc.registryCAs`. Map the registry authority to a PEM bundle path in
the profile config; if the registry advertises a bearer-token service on a different
authority, add a separate entry for that service. Redirect targets using private trust
also need their own entries.

Custom roots augment the normal public roots only for the named authority. dcc keeps
HTTPS and hostname verification enabled, limits redirects, and strips bearer
authorization when a redirect changes origin. CA files are validated eagerly and are
never accepted from downloaded Feature metadata. See [Private Feature Registry
CAs](index.md#private-feature-registry-cas) for the full authority, inheritance, path,
and PEM rules.

## Feature Runtime Metadata

Features can contribute runtime behavior through their
`devcontainer-feature.json` metadata. `dcc build` stores runtime contributions in
the image's `devcontainer.metadata` label, and runtime commands read that label
with `docker image inspect`.

Supported runtime contributions include:

- `remoteEnv`
- `mounts`
- `customizations.dcc.commands`
- `customizations.dcc.state`
- lifecycle hooks
- unsafe runtime settings gated by `--allow-unsafe-runtime`

Feature state is mounted before project state. Feature commands are available
through `dcc run`, and Feature hooks run before the project hook of the same
phase.

Feature state supports the same container-side path variables as project state:
`${containerWorkspaceFolder}` and `${containerCacheFolder}` are resolved before
validation, while `${containerEnv:VAR}` remains deferred until the built image
environment is available. Host-local variables are rejected.

## Feature Commands

Features may expose named shell commands through
`customizations.dcc.commands`. With no command argument, `dcc run` lists project
commands as `:<name>` and Feature commands as `<feature-id>:<name>`.

Unqualified command names are accepted only when unique:

```sh
dcc run lint
dcc run ghcr-io-devcontainers-features-node-1:test
```

Use `dcc exec` for direct command execution. `dcc run /bin/true` looks for a
configured command named `/bin/true`; `dcc exec /bin/true` runs `/bin/true`
directly.

## Feature Lifecycle Hooks

Feature lifecycle hooks use the same supported forms and variable substitution
as project lifecycle hooks. For each hook type, Feature-contributed hooks run
before the `devcontainer.json` hook of that type, in Feature installation order.

Build-preparation hooks (`onCreateCommand`, `updateContentCommand`, and
`postCreateCommand`) run during `dcc build`. Runtime startup hooks
(`postStartCommand`) run inside the PID 1 supervisor when a new profile
container starts. Attach hooks (`postAttachCommand`) run before `dcc attach`
opens its shell or explicit attach command.

A non-zero exit from any hook aborts the current operation and skips subsequent
hooks.

## Supported Feature Properties

The following properties in `devcontainer-feature.json` are read and acted upon
by `dcc`.

| Property | Description |
| --- | --- |
| `options` | Configuration options. Keys are uppercased and passed as environment variables to `install.sh`; user values override defaults. |
| `containerEnv` | Environment variables baked into the image before the Feature's `install.sh` runs. |
| `remoteEnv` | Environment variables passed at runtime. Supports the same runtime substitutions as project `remoteEnv`. |
| `mounts` | Additional runtime mounts. Supports the same substitution as project mounts. |
| `customizations.dcc.commands` | Feature named shell commands invokable through `dcc run`. |
| `customizations.dcc.state` | Feature-contributed persistent state paths. Supports the same container-side path variables and validation as project state; Feature state is mounted before project state. |
| `installsAfter` | Soft ordering hint for Features already in the installation set. |
| `dependsOn` | Hard dependencies. Missing dependencies are added recursively; circular dependencies are an error. |
| `onCreateCommand`, `updateContentCommand`, `postCreateCommand`, `postStartCommand`, `postAttachCommand` | Lifecycle hooks. Feature hooks run before the project hook of the same type. |
| `init`, `entrypoint` | Parsed for compatibility but ignored with a warning because `dcc` owns PID 1. |
| `privileged`, `capAdd`, `securityOpt` | Unsafe runtime settings. Rejected unless the invocation includes `--allow-unsafe-runtime`. |

Feature `containerUser` and `remoteUser` are rejected because the Feature schema
does not permit Features to set users.
