# Architecture

## Overview

`dcc` is a single Rust binary that wraps the Docker CLI to manage ephemeral
devcontainer environments. It adds profile support, durable per-profile caching,
config inheritance via `extends`, and devcontainer Feature installation on top of
the existing devcontainer spec.

---

## Crate Structure

`dcc` is a single binary crate. There is no library layer. The tool is consumed
only as a binary and there is no anticipated reuse as a library. `anyhow::Result<T>`
is used throughout, consistent with the binary-crate convention in STYLE.md.

---

## Module Map

```
src/
  main.rs             Entry point; parses CLI args and dispatches to commands
  cli.rs              clap CLI definitions (Cli struct, Command enum)
  workspace.rs        Workspace discovery (walks ancestor dirs to find .devcontainer)
  profile.rs          ProfileName and ContainerName newtypes; naming logic
  cache.rs            Cache directory creation and path resolution
  docker.rs           Thin wrappers around docker CLI subcommands
  build.rs            dcc build command
  run.rs              dcc run command
  join.rs             dcc join command
  stop.rs             dcc stop command
  config/
    mod.rs            RawConfig and DevcontainerConfig structs; top-level parse fn
    merge.rs          Extends merging algorithm
    resolve.rs        File-level resolution with cycle detection
  features/
    mod.rs            Public API; orchestrates feature download and build-context generation
    oci.rs            Minimal OCI HTTP client for downloading feature artifacts
    context.rs        In-memory tar build context and Dockerfile generation
```

Modules are organized by feature area (STYLE.md). Build, run, join, and stop
live at the top level rather than in a `commands/` subdirectory to avoid a
third level of nesting.

---

## Dependencies

| Crate | Features | Justification |
|---|---|---|
| `clap` | `derive` | CLI argument parsing |
| `serde` | `derive` | Struct deserialization |
| `serde_json` | — | JSON value type; used in feature option maps |
| `json5` | — | JSONC-compatible parsing (trailing commas, `//` comments); devcontainer configs use this format |
| `anyhow` | — | Error handling with context |
| `tokio` | `rt-multi-thread`, `macros`, `process` | Async runtime and subprocess management |
| `reqwest` | `json`, `rustls-tls` | HTTP client for OCI registry; `rustls-tls` avoids OpenSSL for cross-compilation |
| `tar` | — | In-memory tar archive construction for the Docker build context |
| `flate2` | — | gzip decompression of OCI layer blobs |
| `sha2` | — | SHA-256 digest verification of downloaded OCI blobs |
| `indexmap` | `serde` | Ordered map for `features`; preserves declaration order for Feature installation |
| `tracing` | — | Structured logging |
| `tracing-subscriber` | `env-filter` | Log output |

Dev dependencies: `proptest` for property-based tests on the config parser and
merge algorithm; `tempfile` for integration tests that write config files.

`json5` is chosen because the devcontainer spec defines config files as JSONC
(JSON with Comments), and the example configs in this project demonstrate
trailing commas. `json5` is a superset of JSONC and provides serde integration
via `json5::from_str`. A manual preprocessor (strip-comments + strip-trailing-
commas) was considered but rejected because the correct handling of commas inside
strings makes it fragile.

`reqwest` with `rustls-tls` is chosen over native TLS to avoid a system
dependency on OpenSSL, which simplifies the four-target release matrix.

---

## Configuration

### Structs

`RawConfig` is the direct deserialization target. Every field is optional
because any field may be absent in a partial config that is completed by a parent
via `extends`.

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawConfig {
    extends: Option<String>,
    image: Option<String>,
    features: Option<HashMap<String, serde_json::Value>>,
    container_env: Option<HashMap<String, String>>,
    container_user: Option<String>,
    mounts: Option<Vec<String>>,
    forward_ports: Option<Vec<u16>>,
    entrypoint: Option<Vec<String>>,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}
```

The `extra` field collects all unrecognized keys via `#[serde(flatten)]`. After
parsing, dcc iterates `extra` and emits a warning for each key. In `--strict`
mode, the first unrecognized key is a fatal error.

`DevcontainerConfig` is the resolved form after merging and validation. All
collection fields are non-optional (empty by default). `image` is required;
resolution fails if no `image` is present after the full extends chain is merged.

```rust
pub struct DevcontainerConfig {
    pub image: String,
    pub features: IndexMap<String, serde_json::Value>,
    pub container_env: HashMap<String, String>,
    pub container_user: String,                    // default: "dev"
    pub mounts: Vec<String>,
    pub forward_ports: Vec<u16>,
    pub entrypoint: Option<Vec<String>>,
}
```

`IndexMap` is used for `features` to preserve declaration order, which
determines Feature installation order.

The `mounts` field supports only the string form
(`"type=bind,src=...,dst=..."`). The structured object form from the
devcontainer spec is not supported.

### Extends Resolution

Resolution is recursive. A `HashSet<PathBuf>` of canonicalized paths tracks
visited files; if a path is encountered a second time, the chain is circular
and resolution fails with an error naming both the file that triggered the cycle
and the file it tried to extend.

```
load_config(path, strict) -> anyhow::Result<DevcontainerConfig>:
  visited = {}
  raw = load_raw(path, &mut visited, strict)
  validate and convert raw to DevcontainerConfig
    - error if image is None
    - default container_user to "dev"

load_raw(path, visited, strict) -> anyhow::Result<RawConfig>:
  canonical = fs::canonicalize(path)?
  if canonical in visited:
    bail!("{} closes a circular extends chain", canonical.display())
  visited.insert(canonical)
  raw = parse_jsonc(path, strict)?   // emits warnings or errors for extra fields
  if raw.extends is None: return raw
  parent_path = path.parent().join(&raw.extends)
  parent = load_raw(parent_path, visited, strict)?
  return merge(parent, raw)
```

`extends` paths are resolved relative to the file that contains them.

### Merge Algorithm

`merge(parent: RawConfig, child: RawConfig) -> RawConfig`

| Field | Rule |
|---|---|
| `extends` | Dropped; not propagated |
| `image` | Child overwrites parent |
| `features` | Map union; child value wins on key conflict |
| `container_env` | Map union; child value wins on key conflict |
| `container_user` | Child overwrites parent |
| `mounts` | Array union; duplicates removed, parent entries first |
| `forward_ports` | Array union; duplicates removed, parent entries first |
| `entrypoint` | Child overwrites parent (never merged) |

`entrypoint` does not follow the general array-union rule because a child's
entrypoint is a complete command replacement, not an addendum.

### Variable Substitution

Variable substitution is applied to string values in `container_env` and
`mounts` after the full extends chain is merged, before the config is used.

| Variable | Value |
|---|---|
| `${localCacheFolder}` | Absolute path of `.dcc/<profile>` on the host |
| `${containerCacheFolder}` | `/cache` |
| `${localWorkspaceFolder}` | Absolute path of workspace root on the host |
| `${containerWorkspaceFolder}` | `/workspace` |

Substitution is a simple string replacement (all occurrences). An unknown
variable reference is left as-is and triggers a warning.

---

## Workspace Discovery

`find_workspace()` walks from `std::env::current_dir()` through ancestor
directories, stopping at the first directory that contains a `.devcontainer/`
subdirectory. If the filesystem root is reached without finding one, the
function returns an error.

```rust
pub struct Workspace {
    pub root: PathBuf,
}

pub fn find_workspace() -> anyhow::Result<Workspace>
```

---

## Profile Names and Container Names

Three newtypes prevent mixing up the string identifiers that flow through the
system.

```rust
pub struct ProfileName(String);   // e.g. "claude" or "devcontainer"
pub struct ContainerName(String); // e.g. "my-project--claude"
pub struct ImageTag(String);      // same string as ContainerName
```

`ProfileName` encapsulates the path-to-config-file logic:

```rust
impl ProfileName {
    // Returns .devcontainer/<name>.json relative to workspace root.
    // The default "devcontainer" profile follows the same rule and resolves to
    // .devcontainer/devcontainer.json with no special-casing.
    pub fn config_path(&self, workspace: &Workspace) -> PathBuf
}
```

`ContainerName` is derived as `<workspace-basename>--<profile-name>`. It
doubles as the image tag produced by `dcc build` and consumed by `dcc run`.

```rust
impl ContainerName {
    pub fn new(workspace: &Workspace, profile: &ProfileName) -> Self
    pub fn as_image_tag(&self) -> ImageTag
}
```

Docker image tags and container names occupy separate namespaces, so reusing
the same string for both is safe.

---

## Cache Management

```rust
pub struct CacheDir {
    pub host_path: PathBuf,        // <workspace>/.dcc/<profile>
}

impl CacheDir {
    pub fn new(workspace: &Workspace, profile: &ProfileName) -> Self
    pub fn ensure_exists(&self) -> anyhow::Result<()>
    pub fn container_path() -> &'static str  // always "/cache"
}
```

The cache directory is created by `ensure_exists` on `dcc run` if it does not
already exist. It is never deleted automatically.

---

## Docker Integration

### Why CLI, Not API

`dcc run` and `dcc join` require transparent interactive TTY pass-through.
Achieving this via the `bollard` API requires explicit TTY multiplexing and
`SIGWINCH` relay. Shelling out to `docker run -it` and `docker attach` gives
correct TTY behavior by construction, with the Docker client handling all
terminal details natively.

All Docker interaction uses `tokio::process::Command`. Interactive commands
inherit the parent's stdio:

```rust
tokio::process::Command::new("docker")
    .args(...)
    .stdin(Stdio::inherit())
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .spawn()?
    .wait()
    .await?
```

The exit status of every docker subprocess is checked; non-zero codes propagate
as errors, except where noted below.

### dcc build

**No features**: skip image build. Pull and retag:

```
docker pull <image>
docker tag <image> <image-tag>
```

`docker pull` fails if the image does not exist in a remote registry. Images
that were built locally and not pushed to a registry must be handled by adding
features to the config (even an empty set), which causes the Dockerfile path to
be used instead. This limitation should be surfaced in user-facing documentation.

**With features**: pipe the in-memory build context to `docker build`:

```
docker build [--no-cache] --tag <image-tag> -
```

The `-` argument instructs Docker to read the entire build context (including
the Dockerfile) from stdin as a tar archive. No Dockerfile is written to disk.

### dcc run

```
docker run
  --name <container-name>
  --rm
  -it
  --memory <memory>          (default: 4g)
  --cpus <cpus>              (default: 4)
  -e KEY=VALUE ...           (containerEnv after variable substitution)
  -u <containerUser>
  -p <port>:<port> ...       (forwardPorts; host port == container port)
  --mount <spec> ...         (mounts after variable substitution)
  -v <workspace-root>:/workspace
  -v <host-cache-path>:/cache
  --tmpfs /workspace/.dcc
  [--entrypoint <entrypoint[0]>]
  <image-tag>
  [<entrypoint[1:]>]         (remaining elements of configured entrypoint)
  OR
  [<override-command...>]    (user-supplied command args, replaces entrypoint entirely)
```

**Entrypoint resolution**:

1. If the user supplies override args (everything after `--` or the first
   non-flag argument), the first arg becomes `--entrypoint` and the rest become
   post-image arguments. The configured `entrypoint` is ignored.
2. If no override args are given and `entrypoint` is configured, `entrypoint[0]`
   becomes `--entrypoint` and `entrypoint[1:]` become post-image arguments.
3. If no override args and no `entrypoint` is configured, `--entrypoint` is
   omitted and Docker uses the image's default entrypoint.

The `--tmpfs /workspace/.dcc` mount places an empty tmpfs at that path inside
the container, hiding the host `.dcc/` directory (which contains the cache
directories of all profiles) from the container without exposing any data.

The exit status of `docker run` (which reflects the container process's exit
code) is propagated via `std::process::exit`.

### dcc join

```
docker attach <container-name>
```

Reattaches to the running container's main process stdin/stdout/stderr. The
default Docker detach key sequence (Ctrl-P, Ctrl-Q) is inherited from the Docker
client.

### dcc stop

```
docker stop <container-name>
```

If the container does not exist or is not running, `docker stop` returns a
non-zero exit code. `dcc stop` treats this as a success (idempotent). The
distinction is made by inspecting the error output rather than suppressing all
errors.

---

## devcontainer Features

### OCI Artifact Download (`features/oci.rs`)

devcontainer Features are OCI artifacts stored in container registries. A
feature reference like `ghcr.io/devcontainers/features/node:1` is parsed as:

- Registry: `ghcr.io`
- Repository: `devcontainers/features/node`
- Tag: `1`

Download steps:

1. **Authenticate**: Send `GET https://<registry>/v2/`. If the response is 401,
   parse the `WWW-Authenticate: Bearer` header for `realm`, `service`, and
   `scope`. Fetch a token from the realm URL. Tokens are cached in a
   `HashMap<(registry, scope), token>` for the duration of the build. The scope
   is included in the cache key because different repositories on the same
   registry require different scopes.

2. **Fetch manifest**: `GET /v2/<repository>/manifests/<tag>` with
   `Accept: application/vnd.oci.image.manifest.v1+json`. Parse the JSON manifest.
   Identify the layer with media type `application/vnd.devcontainers.layer.v1+tar`.

3. **Download blob**: `GET /v2/<repository>/blobs/<digest>`. The response body
   is a gzip-compressed tar of the feature files.

4. **Verify digest**: Compute SHA-256 of the raw downloaded bytes and compare to
   the digest declared in the manifest. Fail loudly if they do not match. This
   check is not optional.

5. **Extract**: Decompress and untar in memory. Retain `install.sh` and
   `devcontainer-feature.json` (for option defaults).

Feature option values are sourced from the devcontainer config's features map
(e.g., `{"version": "2"}`). Defaults for options not specified by the user come
from `devcontainer-feature.json`. Options are passed to install scripts as
uppercase environment variables (e.g., option `version` becomes `VERSION=2`).

**Feature dependencies** (features that declare other features as prerequisites
in `devcontainer-feature.json`) are not resolved. Features are installed in the
order they appear in the merged config. Users must declare all required features
explicitly. This limitation should be surfaced in user-facing documentation.

### In-Memory Build Context (`features/context.rs`)

The build context is assembled as a `Vec<u8>` using `tar::Builder`. It contains:

**`Dockerfile`**:
```dockerfile
FROM <image>
COPY .dcc-features/ /tmp/.dcc-features/
# Repeated for each feature in declaration order:
RUN chmod +x /tmp/.dcc-features/<id>/install.sh \
 && OPTION_A=value OPTION_B=value ... \
    /tmp/.dcc-features/<id>/install.sh
RUN rm -rf /tmp/.dcc-features/
```

**`.dcc-features/<id>/install.sh`** and **`.dcc-features/<id>/devcontainer-feature.json`**
for each feature.

The `<id>` used as the directory name is a filesystem-safe slug derived from the
feature reference. Because references contain `/`, `:`, and `.`, derive the id
by replacing all non-alphanumeric characters with `-` and lowercasing. For
example, `ghcr.io/devcontainers/features/node:1` becomes
`ghcr-io-devcontainers-features-node-1`. This must be unique across all features
in a given build; if a collision occurs, append a short hash of the original
reference.

The completed `Vec<u8>` is written to the stdin of the `docker build -` process.

---

## Error Handling

`anyhow::Result<T>` is used at every function boundary. Every `?` at a meaningful
boundary carries `.with_context(|| format!("..."))`. Error messages reaching the
user must be diagnosable without access to the source code. Subprocess failures
must include the full command that was attempted.

`unwrap()` and `expect()` are prohibited outside `#[cfg(test)]`. `todo!()` and
`unimplemented!()` are prohibited on any reachable code path.

---

## CLI Definition (`cli.rs`)

```
dcc [--profile <name>] [--strict] <command> [command-flags] [--] [args...]

Commands:
  build   [--no-cache]
  run     [--memory <size>] [--cpus <n>] [--] [command...]
  join
  stop
```

`--profile` defaults to `"devcontainer"`. Implemented with `clap` derive macros.
`trailing_var_arg = true` is set on the `run` subcommand so that clap stops
flag parsing at the first positional argument, allowing `dcc run npm serve` and
`dcc run -- npm serve` to behave identically.

The exit code of `dcc run` mirrors the container process exit code.
All other commands exit 0 on success and 1 on error.

---

## Testing

**Unit tests** (in `#[cfg(test)]` blocks within each file) cover:

- Config parsing: all supported fields, JSONC trailing commas, `//` comments,
  unknown field warnings
- Extends merging: array union, map union, scalar override, `entrypoint` override
  (not merged), empty parent, empty child
- Cycle detection: two-file cycle returns error; three-file chain succeeds;
  three-file cycle returns error
- Variable substitution: all four variables; multiple variables in one string;
  unknown variable passes through with warning
- Container name derivation from various workspace paths
- Profile config path resolution including the default `"devcontainer"` profile
- Feature reference parsing: valid references, missing tag, invalid registry
- Feature option env-var generation: lowercase option names uppercased

**Property-based tests** (`proptest`, in the same `#[cfg(test)]` blocks) cover:

- Config merging: merging a config with an empty config returns the original;
  merging is stable under repeated application
- Feature reference parsing: arbitrary strings do not panic

**Integration tests** (`tests/`) invoke the compiled `dcc` binary via
`std::process::Command` and test:

- Error on missing `.devcontainer` directory
- Error on missing profile config file
- Error on circular `extends`
- `--strict` rejects unknown fields; default mode warns and continues
- `--` separator passes remaining args as command override

Integration tests that require a live Docker daemon are annotated `#[ignore]`.
