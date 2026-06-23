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
  forward.rs          Host-side TCP relay for forwardPorts
  config/
    mod.rs            RawConfig and DevcontainerConfig structs; top-level parse fn
    merge.rs          Extends merging algorithm
    resolve.rs        File-level resolution with cycle detection
    vars.rs           Variable substitution (${localCacheFolder} etc.); container path constants
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
| `tokio` | `rt-multi-thread`, `macros`, `process`, `io-util`, `net`, `time` | Async runtime, subprocess management, TCP listeners for port forwarding, and timer for container readiness polling |
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
    remote_env: Option<HashMap<String, String>>,
    container_user: Option<String>,
    mounts: Option<Vec<String>>,
    forward_ports: Option<Vec<u16>>,
    command: Option<Vec<String>>,
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
    pub remote_env: HashMap<String, String>,
    pub container_user: Option<String>,            // None → use image's USER directive
    pub mounts: Vec<String>,
    pub forward_ports: Vec<u16>,
    pub command: Option<Vec<String>>,
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
    - container_user kept as Option<String>; None means use the image's USER directive

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
| `remote_env` | Map union; child value wins on key conflict |
| `container_user` | Child overwrites parent |
| `mounts` | Array union; duplicates removed, parent entries first |
| `forward_ports` | Array union; duplicates removed, parent entries first |
| `command` | Child overwrites parent (never merged) |

`command` does not follow the general array-union rule because a child's
command is a complete replacement, not an addendum.

### Variable Substitution

Substitution runs in two contexts with different variable sets.

**`containerEnv`** (devcontainer.json and feature) is baked into the image at
build time, so only container-side constants are substituted:

| Variable | Value |
|---|---|
| `${containerCacheFolder}` | `/cache` |
| `${containerWorkspaceFolder}` | `/workspace` |

**Runtime-applied properties** — `remoteEnv`, `mounts`, the container command
(`dcc run` scripts / `dcc exec` args), and the lifecycle commands
(`initializeCommand` plus the in-container hooks) — additionally substitute:

| Variable | Value |
|---|---|
| `${localCacheFolder}` | Absolute path of `.dcc/<profile>` on the host |
| `${localWorkspaceFolder}` | Absolute path of the workspace root on the host |
| `${containerCacheFolder}` | `/cache` |
| `${containerWorkspaceFolder}` | `/workspace` |
| `${localEnv:VAR}` / `${localEnv:VAR:default}` | Host process env var `VAR` |
| `${containerEnv:VAR}` / `${containerEnv:VAR:default}` | `VAR` from the **built image** env (base image `ENV` + baked `containerEnv`), plus the configured user's runtime `HOME` and `USER` |

Resolution timing differs. The path variables and `${localEnv:…}` are resolved at
config-load (`vars::apply_substitution`) since they are knowable on the host.
`${containerEnv:…}` is **deferred** there (left intact, not flagged as unknown)
and resolved at run time in `exec.rs` by `vars::resolve_container_env` against the
image's `Config.Env` (read via `docker::inspect_image_env`). The resolved literal
is placed into the `-e`/`--mount`/command/hook strings, so it lands in the
container config env and is inherited uniformly (PID 1, its children, and
`docker exec`). `${containerEnv:…}` does not see `remoteEnv` values.

`HOME` and `USER` are set by the container runtime (from `/etc/passwd` and the
`-u` user), not baked into `Config.Env`. When any runtime-applied field references
`${containerEnv:…}`, `exec.rs` therefore probes the configured user's `HOME`/`USER`
once (`docker::probe_user_env` — a throwaway `docker run … sh -c 'echo $HOME; id -un'`)
and merges them into the resolution map. The probe is gated on actual use
(`exec::references_container_env`) so configs that don't use containerEnv pay nothing;
a probe failure is a warning, leaving them unresolved (which then errors below).

A `${containerEnv:VAR}` that is **undefined or empty** with no `:default` is a hard
error (`resolve_container_env` returns `Result`), so a typo or an unsupported variable
(e.g. `${containerEnv:HOSTNAME}`) fails loudly instead of silently becoming empty. An
explicit empty default (`${containerEnv:VAR:}`) opts back into an empty value. An
undefined `${localEnv:…}` still resolves to its `:default` or the empty string. Local
and env-namespace variables are not substituted inside a `containerEnv` value (it is
build-time). Any other unknown `${…}` is left as-is and triggers a warning; the run
path additionally prints a user-facing warning for unresolved references left in a
mount or `remoteEnv`.

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
}
```

The cache directory is created by `ensure_exists` on `dcc run` if it does not
already exist. It is never deleted automatically. `dcc build` does not create
or write to the cache directory; all persistent build output is stored in the
Docker image itself (see `devcontainer.metadata` label below).

The container-side mount path (`/cache`) is defined as the constant
`CONTAINER_CACHE` in `config/vars.rs`, which also defines `CONTAINER_WORKSPACE`
(`/workspace`). Both constants are shared between variable substitution and the
`docker run` argument construction in `run.rs`.

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

**No features AND no containerUser AND no containerEnv AND no forwardPorts** (fast path): skip image build. Pull and retag:

```
docker pull <image>
docker tag <image> <image-tag>
```

`docker pull` fails if the image does not exist in a remote registry. Images
that were built locally and not pushed to a registry must be handled by adding
features, a `containerUser`, `containerEnv`, or `forwardPorts` to the config,
which causes the Dockerfile path to be used instead. This limitation should be
surfaced in user-facing documentation.

**With features OR containerUser OR containerEnv OR forwardPorts**: pipe the in-memory build context to `docker build`:

```
docker build [--no-cache] [--label devcontainer.metadata=<json>] --tag <image-tag> -
```

The `-` argument instructs Docker to read the entire build context (including
the Dockerfile) from stdin as a tar archive. No Dockerfile is written to disk.

When features contribute runtime properties (mounts, command, remoteEnv),
`dcc build` passes `--label devcontainer.metadata=<json>` to `docker build`.
The label value is a JSON array with one entry per contributing feature and is
stored inside the image. `dcc run` reads it back via `docker image inspect`
rather than relying on any local file, making the image self-describing and
portable across machines.

### dcc run

`dcc run` uses a three-phase sequence: start detached, set up port forwarders,
then attach interactively.

**Phase 1 — pre-flight checks and argument construction**

Before starting Docker, `dcc run`:

1. Calls `docker image inspect` on the image tag to read its
   `devcontainer.metadata` label, if present. The label JSON is parsed into a
   `FeatureRuntimeConfig` (mounts, command, remoteEnv). A missing label is
   treated as no feature runtime contributions; a malformed label is a fatal
   error. It also reads the image's `Config.Env` (`{{json .Config.Env}}`, via
   `docker::inspect_image_env`) to resolve `${containerEnv:VAR}` references in the
   runtime properties.
2. Calls `fs::create_dir_all` for any bind mount whose `src=` path falls under
   the host cache directory. Docker requires bind mount source paths to exist on
   the host before the container starts.

**Phase 2 — detached container start**

The container is started with `-dit` (detached, interactive, TTY pre-allocated):

```
docker run
  --name <container-name>
  --rm
  -dit
  --memory <memory>          (default: 4g)
  --cpus <cpus>              (default: 2)
  [-u <containerUser>]       (omitted when containerUser is not set)
  -e KEY=VALUE ...           (remoteEnv after variable substitution)
  -e KEY=VALUE ...           (feature remoteEnv after template substitution)
  --mount <spec> ...         (mounts after variable substitution)
  -v <workspace-root>:/workspace
  -v <host-cache-path>:/cache
  --tmpfs /workspace/.dcc
  --entrypoint tail          (keep-alive; the user command runs via docker exec)
  <image-tag>
  -f /dev/null
```

The container's PID 1 is a keep-alive process (`tail -f /dev/null`) so it stays
running independent of the user command. This is deliberate: making the user
command PID 1 and attaching to it fails for commands that exit quickly (e.g.
`ls`) — the container is gone before the readiness poll observes it as running,
and `docker attach` would not show output produced before the attach. The user
command instead runs in the foreground via `docker exec` (phase 4). `dcc run`
polls `docker inspect` at 100 ms intervals (up to 10 s) until the keep-alive
container reports as running.

Once the container is running, the container-side lifecycle hooks
(`onCreateCommand` through `postAttachCommand`) run in spec order before port
forwarding. The `--skip-lifecycle` flag on `dcc exec` skips every lifecycle script —
the host `initializeCommand` and all in-container hooks — and prints a warning
naming each skipped script (`exec::skipped_hook_warnings` builds the list in
execution order), so a misbehaving script can be bypassed when debugging without
silently dropping anything.

Note that `forwardPorts` no longer translates to `-p` flags. Publishing ports
with Docker's `-p` mechanism routes traffic through the Docker bridge network,
so the container application sees connections as coming from the bridge gateway
IP rather than `127.0.0.1`. Port forwarding is handled separately in phase 3.

**Phase 3 — port forwarding**

For each port in `forwardPorts`, `dcc run` binds a `TcpListener` on
`127.0.0.1:<port>` on the host and spawns a Tokio task (see Port Forwarding
below). The listeners are bound before the command runs so that ports are ready
as soon as the session begins.

**Phase 4 — foreground command**

```
docker exec -i [-t] -u <containerUser> -w /workspace <container-name> <command...>
```

The user command runs in the foreground via `docker exec`, with stdio inherited,
so output streams live and the command's real exit code is returned. This works
uniformly for one-off commands (`ls`) and interactive shells (`bash`). `-t` is
requested only when dcc's own stdin is a terminal, so non-interactive use (pipes,
CI) still works. The exit status is propagated via `std::process::exit`.

**Phase 5 — teardown**

After the foreground command returns, all relay task handles are aborted and the
keep-alive container is stopped (`docker stop`; `--rm` then removes it). In-flight
`docker exec nc` processes terminate as the container goes away; the abort
releases the host-side port bindings.

**Command resolution** (descending priority):

1. If the user supplies override args (everything after `--` or the first
   non-flag argument), they are run in the foreground via `docker exec` against
   the keep-alive container. All configured commands are ignored.
2. If `command` is set in `devcontainer.json`, it is used. A warning is
   emitted if any feature also declared a command.
3. If no devcontainer.json command but a feature contributed one (from the
   image label), that command is used.
4. If none of the above apply, `--entrypoint` is omitted and Docker uses the
   image's default.

The `--tmpfs /workspace/.dcc` mount places an empty tmpfs at that path inside
the container, hiding the host `.dcc/` directory from the container.

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

## Port Forwarding

The devcontainer spec distinguishes between *publishing* and *forwarding* ports.
Docker's `-p HOST:CONTAINER` flag *publishes* a port: traffic arrives at the
container via the Docker bridge network and the application sees the source
address as the bridge gateway (e.g. `172.17.0.1`), not `127.0.0.1`. An
application that binds only to `localhost` rejects such connections.

*Forwarding* means routing traffic through the container's own loopback
interface so the application sees the source address as `127.0.0.1`. `dcc`
implements this using a host-side TCP relay (`forward.rs`) and `nc` (netcat)
running inside the container.

### Relay architecture (`forward.rs`)

For each port in `forwardPorts`, `dcc run` binds a `TcpListener` on
`127.0.0.1:<port>` on the host and spawns a long-running Tokio task
(`relay_port`). For each accepted connection the task spawns a short-lived
connection-handler task (`handle_connection`) and immediately resumes
accepting.

`handle_connection` opens a tunnel by spawning:

```
docker exec -i <container-name> nc 127.0.0.1 <port>
```

`nc` runs inside the container and connects to `127.0.0.1:<port>` on the
container's own loopback interface. `docker exec -i` pipes the process's
stdin/stdout back to the host. The handler then calls `tokio::io::copy` in
both directions concurrently via `tokio::select!`:

```
host TCP socket  ←→  docker exec -i nc  ←→  app (127.0.0.1:<port> inside container)
```

Because `nc` connects from within the container, the application sees the
connection as originating from `127.0.0.1`, not from the Docker bridge.

When either direction closes (connection dropped, app closed the socket, or
`nc` exits), `tokio::select!` cancels the other direction, `nc` is killed,
and the handler task exits.

### Why nc (netcat)

`nc` is the lowest-common-denominator TCP client available in virtually all
Linux base images. It does not require any special privileges, does not need
a daemon running inside the container, and its stdin/stdout are directly
usable as a byte stream — exactly what `docker exec -i` pipes.

`dcc build` installs `nc` automatically using a cross-distro Dockerfile `RUN`
step (see In-Memory Build Context above). The step short-circuits if `nc` is
already present and tries each package manager in turn:

| Package manager | Package installed |
|---|---|
| `apt-get` (Debian/Ubuntu) | `netcat-openbsd` |
| `apk` (Alpine) | `netcat-openbsd` |
| `yum` (RHEL/CentOS) | `nmap-ncat` |
| `dnf` (Fedora/RHEL 8+) | `nmap-ncat` |

The fast-path in `dcc build` (pull + retag, no Dockerfile) is bypassed when
`forwardPorts` is non-empty, since the nc installation step requires a
Dockerfile build.

### Handle lifetime and cleanup

The relay tasks hold `JoinHandle`s returned by `tokio::spawn`. `dcc run`
stores all handles in a `Vec` and calls `handle.abort()` on each after
`docker attach` returns. Aborting the listener task causes `listener.accept()`
to resolve with an error and the loop exits, releasing the port binding.

Per-connection tasks are spawned without retained handles. They are
genuinely short-lived (they exit when the connection closes) and
self-cleaning (the `nc` subprocess exits as soon as the container is gone).
Holding handles for these tasks would require a `JoinSet` or equivalent and
add complexity for no observable benefit given their bounded lifetime.

---

## devcontainer Features

### Feature resolution (`features/mod.rs`)

`build_context` runs three phases before assembling the Docker build context:

**Phase 1 — dependency resolution**: Starting from the user's feature list,
each feature's `devcontainer-feature.json` is read. Features declared in
`dependsOn` that are not already present are appended to the work queue and
processed recursively. A `HashSet` of enqueued references prevents re-queueing.
When a dependency is already present with different options, the existing options
are kept and a warning is emitted.

**Phase 2 — topological sort** (Kahn's algorithm): A directed graph is
constructed from `dependsOn` edges (hard) and `installsAfter` edges (soft).
`installsAfter` is matched by the feature's `id` field from its metadata.
Independent features are processed in their original declaration order (the
`IndexMap` insertion order is used as the tiebreaker). A cycle in the graph is
a fatal error.

**Phase 3 — context assembly**: In topological order, each feature contributes:
- `containerEnv` → substituted with container-only variables, written to `FeatureContext.container_env` (becomes Dockerfile `ENV`)
- `remoteEnv` → stored as raw templates in the feature's label entry; substitution is applied at `dcc run` time
- `mounts` → stored as JSON objects in the feature's label entry; converted to `--mount` template strings and substituted at `dcc run` time
- `command` → stored in the feature's label entry; last feature wins, warning emitted on clobber

Features that contribute at least one runtime property (mounts, command, or
remoteEnv) get an entry in the `devcontainer.metadata` label JSON array. Features
that contribute only build-time properties (`containerEnv`, `options`) are omitted
from the label. The label is embedded in the image via `docker build --label`.

### Supported feature properties

| Property | Description |
|---|---|
| `options` | Configuration options. Keys are uppercased and passed as environment variables to `install.sh`. |
| `command` | Array of strings passed to Docker as `--entrypoint` when the container starts. Last feature wins; warning emitted on clobber. |
| `containerEnv` | Environment variables baked into the image as Dockerfile `ENV` directives. Only container-side variables are substituted. |
| `remoteEnv` | Environment variables passed as `-e` runtime flags to `docker run`. Stored as raw templates; substituted at `dcc run` time. |
| `mounts` | Additional mounts attached at `dcc run` time. |
| `installsAfter` | Soft ordering hint. Feature IDs that this feature should be installed after (if present). |
| `dependsOn` | Hard dependencies. Missing dependencies are added to the installation set automatically. |

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

5. **Extract**: Decompress and untar in memory. Retain `install.sh`,
   `devcontainer-feature.json`, and any other regular files (e.g. helper scripts
   in `library_scripts/`). The archive root is determined by the first regular
   file's path prefix; paths are preserved relative to that root.

Feature option values are sourced from the devcontainer config's features map
(e.g., `{"version": "2"}`). Defaults for options not specified by the user come
from `devcontainer-feature.json`. Options are passed to install scripts as
uppercase environment variables (e.g., option `version` becomes `VERSION=2`).

### In-Memory Build Context (`features/context.rs`)

The build context is assembled as a `Vec<u8>` using `tar::Builder`. It contains:

**`Dockerfile`**:
```dockerfile
FROM <image>
# devcontainer.json containerEnv directives (sorted by key; omitted when empty):
ENV DC_VAR='value'
# Only present when features are configured:
COPY .dcc-features/ /tmp/.dcc-features/
# Repeated for each feature in installation order:
ENV CONTAINER_VAR='value'          # only if feature declares containerEnv
RUN chmod +x /tmp/.dcc-features/<id>/install.sh \
 && OPTION_A=value OPTION_B=value ... \
    /tmp/.dcc-features/<id>/install.sh
RUN rm -rf /tmp/.dcc-features/
# Only present when containerUser is set and is not "root":
RUN id '<user>' >/dev/null 2>&1 \
 || useradd -m -s /bin/sh '<user>' \
 || adduser -D -s /bin/sh '<user>'
# Only present when forwardPorts is non-empty (see Port Forwarding below):
RUN command -v nc >/dev/null 2>&1 \
 || (command -v apt-get >/dev/null 2>&1 && apt-get update -qq && apt-get install -y --no-install-recommends netcat-openbsd) \
 || (command -v apk     >/dev/null 2>&1 && apk add --no-cache netcat-openbsd) \
 || (command -v yum     >/dev/null 2>&1 && yum install -y nmap-ncat) \
 || (command -v dnf     >/dev/null 2>&1 && dnf install -y nmap-ncat)
```

The devcontainer.json `containerEnv` `ENV` directives appear immediately after
`FROM`, before any feature blocks. Feature `containerEnv` `ENV` directives are
emitted immediately before each feature's `RUN` step so the variables are
available to `install.sh` and remain set in the image for all subsequent layers.

The user-creation step is idempotent (`id` short-circuits when the user already
exists) and cross-distro compatible: `useradd` covers Debian/Ubuntu/RHEL/Fedora;
`adduser -D` covers Alpine/BusyBox. It runs after features so a feature that
already creates the user does not cause a conflict.

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
dcc [--strict] [-p/--profile <name>] <command> [-p/--profile <name>] [command-flags] [--] [args...]

Commands:
  build  [--no-cache]
  run    [--memory <size>] [--cpus <n>] [--] [command...]
  join
  stop
```

`--profile` (`-p`) is a clap **global argument** declared once on `Cli` and
read from the single `Cli::profile` field. As a global argument it is accepted
in both positions — `dcc -p claude run` and `dcc run -p claude` are equivalent —
so users are not forced to remember whether the flag precedes or follows the
subcommand. Earlier versions declared `-p` on each subcommand to allow it after
the subcommand; the global argument supersedes that, supporting both orderings
with no duplication. `--profile` defaults to `"devcontainer"`. (For commands
like `dcc exec`/`dcc run` whose trailing arguments form the in-container
command, `-p` must precede the first positional argument, otherwise it is passed
through to that command.)

`--strict` is declared on `Cli` but is **not** global, so it is accepted only
before the subcommand (`dcc --strict build`). It affects config parsing, which
applies identically across all subcommands.

Implemented with `clap` derive macros. `trailing_var_arg = true` is set on the
`run` subcommand so that clap stops flag parsing at the first positional
argument, allowing `dcc run npm serve` and `dcc run -- npm serve` to behave
identically.

The exit code of `dcc run` mirrors the container process exit code.
All other commands exit 0 on success and 1 on error.

---

## Testing

**Unit tests** (in `#[cfg(test)]` blocks within each file) cover:

- Config parsing: all supported fields, JSONC trailing commas, `//` comments,
  unknown field warnings
- Extends merging: array union, map union, scalar override, `command` override
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
