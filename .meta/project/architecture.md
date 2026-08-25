# Architecture

## Overview

`dcc` is a single Rust binary that wraps the Docker CLI to manage profile-specific
devcontainer environments. It adds profile support, durable per-profile caching,
durable and one-shot runtime lifecycle commands, config inheritance via `extends`, and
devcontainer Feature installation on top of the existing devcontainer spec.

---

## Crate Structure

`dcc` is a single binary crate. There is no library layer. The tool is consumed
only as a binary and there is no anticipated reuse as a library. `anyhow::Result<T>`
is used throughout, consistent with the binary-crate convention in
`.meta/project/rust-style.md`.

---

## Module Map

```
src/
  main.rs             Entry point; parses CLI args and dispatches to commands
  cli.rs              clap CLI definitions (Cli struct, Command enum)
  workspace.rs        Workspace discovery (walks ancestor dirs to find .devcontainer)
  profile.rs          Profile discovery plus ProfileName, ContainerId, and ContainerName newtypes
  cache.rs            Cache directory creation and path resolution
  docker.rs           Thin wrappers around docker CLI subcommands
  build.rs            dcc build command
  feature.rs          dcc feature profile config edit command
  run.rs              dcc run command
  supervisor.rs       In-container PID 1 lifecycle supervisor (scripts + host-side rt dir)
  seed.rs             State seeding from the image (hydration, dcc.seed label, ledger)
  stop.rs             dcc stop command (graceful / --now / --kill variants)
  uid.rs              updateRemoteUserUID remap planning (host uid/gid, no-op conditions, Dockerfile block)
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

Modules are organized by feature area (`.meta/project/rust-style.md`). Build, run, and stop
live at the top level rather than in a `commands/` subdirectory to avoid a
third level of nesting.

---

## Dependencies

| Crate | Features | Justification |
|---|---|---|
| `clap` | `derive` | CLI argument parsing |
| `serde` | `derive` | Struct deserialization |
| `serde_json` | `preserve_order` | JSON value type; used in feature option maps and profile feature edits where object order should remain stable |
| `json5` | — | JSONC-compatible parsing (trailing commas, `//` comments); devcontainer configs use this format |
| `anyhow` | — | Error handling with context |
| `tokio` | `rt-multi-thread`, `macros`, `process`, `io-util`, `net`, `time` | Async runtime, subprocess management, TCP listeners for port forwarding, and timer for container-exists polling (`wait_for_running`) |
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

`RawConfig` is the direct deserialization target. Every field is optional because any
field may be absent in a partial config that is completed by a parent via
`customizations.dcc.extends`. The parser accepts the official and dcc-owned fields used
by the current implementation, including:

```rust
struct RawConfig {
    extends: Option<String>,
    name: Option<String>,
    image: Option<String>,
    build: Option<BuildConfig>,
    features: Option<HashMap<String, serde_json::Value>>,
    container_env: Option<HashMap<String, String>>,
    remote_env: Option<HashMap<String, String>>,
    container_user: Option<String>,
    mounts: Option<Vec<String>>,
    run_args: Option<Vec<String>>,
    privileged: Option<bool>,
    cap_add: Option<Vec<String>>,
    security_opt: Option<Vec<String>>,
    forward_ports: Option<Vec<u16>>,
    ports_attributes: Option<HashMap<String, PortAttributes>>,
    other_ports_attributes: Option<PortAttributes>,
    override_command: Option<bool>,
    update_remote_user_uid: Option<bool>,
    workspace_folder: Option<String>,
    workspace_mount: Option<serde_json::Value>,
    initialize_command: Option<LifecycleCommand>,
    on_create_command: Option<LifecycleCommand>,
    update_content_command: Option<LifecycleCommand>,
    post_create_command: Option<LifecycleCommand>,
    post_start_command: Option<LifecycleCommand>,
    post_attach_command: Option<LifecycleCommand>,
    customizations: Option<Customizations>,
    extra: HashMap<String, serde_json::Value>, // serde flatten
}
```

The `extra` field collects all unrecognized keys via `#[serde(flatten)]`. After
parsing, dcc iterates `extra` and emits a warning for each key. In `--strict`
mode, the first unrecognized key is a fatal error.

`DevcontainerConfig` is the resolved form after merging and validation. All collection
fields are non-optional (empty by default). Exactly one of `image` or official `build`
is required after the full extends chain is merged.

```rust
pub struct DevcontainerConfig {
    pub name: Option<String>,
    pub image: Option<String>,
    pub build: Option<BuildConfig>,
    pub features: IndexMap<String, serde_json::Value>,
    pub container_env: HashMap<String, String>,
    pub remote_env: HashMap<String, String>,
    pub container_user: String, // defaults to "dev"
    pub mounts: Vec<String>,
    pub run_args: Vec<String>,
    pub unsafe_runtime: UnsafeRuntimeConfig,
    pub forward_ports: Vec<u16>,
    pub ports_attributes: HashMap<String, PortAttributes>,
    pub other_ports_attributes: Option<PortAttributes>,
    pub override_command: Option<bool>,
    pub update_remote_user_uid: bool, // defaults to true
    pub workspace_folder: String,
    pub workspace_mount: Option<serde_json::Value>,
    pub state: Vec<StateEntry>,
}
```

`IndexMap` is used for `features` to preserve declaration order, which
determines Feature installation order.

The top-level `mounts` field currently supports the string form
(`"type=bind,src=...,dst=..."`). Feature `mounts` support the official object form and
are converted to Docker `--mount` strings through Feature metadata parsing.

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
    - error unless exactly one of image or build is present
    - container_user defaults to "dev"
    - compatibility fields such as runArgs, workspaceFolder, and port attributes default to empty/default values

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
| `name` | Child overwrites parent |
| `image` | Child overwrites parent |
| `features` | Map union; child value wins on key conflict |
| `container_env` | Map union; child value wins on key conflict |
| `remote_env` | Map union; child value wins on key conflict |
| `container_user` | Child overwrites parent |
| `mounts` | Array union; duplicates removed, parent entries first |
| `run_args` | Array union; duplicates removed, parent entries first |
| `privileged`, `cap_add`, `security_opt` | Child overwrites scalar `privileged`; arrays union |
| `forward_ports` | Array union; duplicates removed, parent entries first |
| `ports_attributes` | Map union; child value wins on key conflict |
| `other_ports_attributes`, `override_command`, `update_remote_user_uid`, `workspace_folder`, `workspace_mount` | Child overwrites parent |

Lifecycle hook fields are not merged as arrays; the child value wins for each hook.

### Variable Substitution

Substitution runs in two contexts with different variable sets.

**`containerEnv`** (devcontainer.json and feature) is baked into the image at
build time, so only container-side constants are substituted:

| Variable | Value |
|---|---|
| `${containerCacheFolder}` | `/cache` |
| `${containerWorkspaceFolder}` | `/workspace` |

**Runtime-applied properties** — `remoteEnv`, `mounts`, `runArgs`, `workspaceFolder`,
the container command (`dcc run` scripts / `dcc exec` args), and supported
in-container lifecycle hooks — additionally substitute:

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
a probe failure is a warning, leaving the keys absent. A reference without a default
then produces the same missing-variable error as any other absent image variable.

An absent `${containerEnv:VAR}` must have an explicit `:default` or resolution fails
with an error naming the missing variable and consumer. A default may be empty. A
present-but-empty value remains empty and does not use a default, while a present
non-empty value always wins. Consumer validation runs afterward: notably, state paths
still reject empty, relative, root, overlapping, and reserved resolved paths.
`${localEnv:…}` retains its separate Dev Container absent/default behavior. Local and
env-namespace variables are not substituted inside a `containerEnv` value (it is
build-time). Any other unknown `${…}` is left as-is and triggers a warning; the run path
additionally prints a user-facing warning for unresolved references left in a mount or
`remoteEnv`. The durable rationale and exact matrix are recorded in decision 0006;
decision 0005 preserves the superseded upstream-compatible policy as history.

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

## Profile Names and Container Identity

Three newtypes prevent mixing up the string identifiers that flow through the
system.

```rust
pub struct ProfileName(String);   // e.g. "claude" or "devcontainer"
pub struct ContainerId(String);   // e.g. "dcc-abc123def456--claude"
pub struct ContainerName(String); // Docker-visible name, from config `name`
pub struct ImageTag(String);      // same string as ContainerId
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

`dcc profile list` scans only direct entries in the workspace's `.devcontainer`
directory. Exact `.json` files and symlinks to files map to the suffix-stripped profile
name; directories, broken symlinks, other extensions, and the empty name are ignored.
The typed result is sorted by name before both renderers run. Text escapes control
characters and backslashes, prints one name per line, and annotates `devcontainer` as
`(default)`; JSON emits ordered `name`, `config`, and `default` fields with normal JSON
escaping. The command dispatches before normal `-p` resolution and never loads
configuration contents or invokes Docker.

`ContainerId` is derived as `dcc-<12hex>--<profile-name>`, where `<12hex>` is
derived from the stable workspace identity. It doubles as the image tag produced
by `dcc build` and consumed by `dcc run`, and it is what `dcc id` prints.

```rust
impl ContainerId {
    pub fn new(workspace: &Workspace, profile: &ProfileName) -> Self
    pub fn as_image_tag(&self) -> ImageTag
}
```

`ContainerName` is the Docker-visible container name. It resolves from the
devcontainer `name` field when present, falling back to `ContainerId`. Invalid
Docker name characters are converted to `-`, repeated `-` characters are
collapsed, invalid edge characters are trimmed, and a warning is emitted when the
configured value changes. If no valid characters remain, it falls back to
`ContainerId`.

---

## Cache Management

```rust
pub struct CacheDir {
    pub host_path: PathBuf,        // <workspace>/.dcc/<profile>
}

impl CacheDir {
    pub fn new(workspace: &Workspace, profile: &ProfileName) -> Self
    pub fn ensure_exists(&self) -> anyhow::Result<()>
    pub fn plan_state_mounts(&self, state: &[StateEntry]) -> Vec<StateMount>
    pub fn prepare_state_mounts(&self, mounts: &[StateMount]) -> anyhow::Result<()>
}
```

The cache directory is created by `ensure_exists` for build preparation and runtime
commands if it does not already exist. It is never deleted automatically. The
managed `.dcc/` root receives a `.gitignore` containing `*` when no entry already
exists at that path, keeping generated profiles, runtime assets, seed ledgers, caches,
and state out of Git status. Existing `.dcc/.gitignore` entries are never modified.
Build preparation may hydrate declared state into the profile directory and record a
seed ledger alongside it (see `devcontainer.metadata` label below).

Declared `customizations.dcc.state` entries are validated as absolute container
paths after config merge and container-side substitution. They reject unresolved
host-local variables, parent/child overlaps, `/`, `..`, and reserved container
paths. Runtime `${containerEnv:VAR}` references are resolved after image/user
environment probing and then validated again.

State bind mounts mask image content with an empty host source, so reserved-path
guards prevent masking a path whose loss would break the container or `dcc`
itself. Two tiers are enforced at both config-load normalization and
post-`${containerEnv:VAR}` resolution:

- **Subtree** (path and all descendants): `/proc`, `/sys`, `/dev`, `/tmp`,
  `/run`, `/var/run`, `/var/lock`, `/boot`, `/bin`, `/sbin`, `/lib`, `/lib32`,
  `/lib64`, `/libx32`, `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/lib32`,
  `/usr/lib64`, `/usr/libx32`, `/etc`, `/workspace/.dcc`, `/cache`, and
  `/usr/local/share/dcc`. `/bin` and `/usr/bin` are both listed because
  merged-`usr` distributions symlink one to the other, so blocking one spelling
  does not block the other. `/etc` is a subtree block because empty-file state
  corrupts `passwd`/`group`/`nsswitch.conf`; the error names the lifecycle-hook
  alternative. `/cache` is the profile cache mount itself (self-nesting), and
  `/usr/local/share/dcc` holds `dcc`'s baked supervisor scripts; startup hook
  assets are mounted beneath `/usr/local/share/dcc/rt` for each launch.
- **Exact** (bare path only; subdirectories stay valid): `/usr`, `/var`,
  `/home`, `/root`, `/opt`, `/workspace`, `/srv`, `/mnt`, `/media`. Legitimate
  cache targets such as `/usr/local/cargo`, `/var/cache/apt`, `/home/dev/.cargo`,
  `/root/.cargo`, and `/workspace/target` nest beneath these and remain accepted.

The guards are textual; a state path that is a symlink in the image resolving
outside itself is not detected in `normalize_state_path`.

### State seeding

Bind mounts mask image content with an empty host source, so data a Feature
`install.sh`, a `Dockerfile` layer, or an official `build` source placed at a
declared state path is invisible at runtime. `dcc build` seeds declared state
from the image (`src/seed.rs`): it runs one short-lived container on the
finished image with the state mounts **not** applied and the host state root
mounted at `/dcc-seed`, then copies each declared path's image content into the
host state directory with `tar` inside the container. Modes and symlinks are
preserved; for non-root `containerUser` profiles, hydration re-owns copied state
to that user so bind-mounted state remains writable even when Dockerfile layers
created the content before `updateRemoteUserUID` remapped the user. A host-side
`docker cp` is deliberately avoided because it would land every file owned by
the invoking host user.

A `dcc.seed` image label (alongside `devcontainer.metadata` and `dcc.version`)
carries the resolved seed manifest — per entry the container path, kind, and
content digest — read back with the same `docker::inspect_image_label_value`
idiom. The label lets `dcc build --dry-run` report planned seeding and lets the
runtime guard compare `build_id`s without re-digesting content.

Hydration decisions are driven by a host-side ledger at
`.dcc/<profile>.seed.json` (a sibling of the profile cache directory, outside the
`/cache` mount so container-side code cannot reach it). Each entry records
`seed_digest` (digest of what `dcc` wrote) and `build_id` (image identity). The
ledger is authoritative: `dcc` never infers intent from directory emptiness, so
an empty seeded directory and an unseeded one are distinguishable, and empty file
state remains legitimate. Digest identity includes raw file bytes and, inside
directories, normalized relative paths and symlink targets. Traversal order,
ownership, timestamps, and permission modes are intentionally normalized or excluded
to keep equivalent content stable across hosts.

Build ordering:

```
build image
  -> resolve state paths (inspect_image_env / probe_user_env / resolve_runtime_state)
  -> hydrate unseeded or safely-refreshable entries   [seed.rs]
  -> prepare_state_mounts
  -> start build-prep container
  -> onCreateCommand / updateContentCommand / postCreateCommand
```

Hydrating before build-prep is what fixes the hook blind spot: `onCreateCommand`
observes install-time content. Hydration needs its own container because
build-prep runs *with* state mounts attached and therefore cannot see the image
content.

Re-seed policy (`dcc build`): if the host state digest matches the recorded
`seed_digest`, skip re-hydration; if it differs, preserve the user's data and
warn (naming the path, the recorded digest, and `--reseed-state`); `dcc build
--reseed-state` overwrites differing host state. The policy is all-or-nothing
across every declared state path.

Runtime guard (`dcc start`/`run`/`exec`/`attach`): compare the ledger `build_id`
against the image `dcc.seed` label and warn on mismatch; hydrate only entries
with no ledger record (wiped-`.dcc` recovery). Content re-digesting is off the
hot path. The guard is best-effort — a stale ledger never blocks the runtime.

Each accepted state path is planned as a bind mount rooted below
`<workspace>/.dcc/<profile>/state/` using the normalized container path. Directory
state creates the host directory; file state creates the host parent directory and
an empty file when absent.

The container-side cache mount path (`/cache`) is defined as the constant
`CONTAINER_CACHE` in `config/vars.rs`, which also defines `CONTAINER_WORKSPACE`
(`/workspace`). Both constants are shared between variable substitution and the
`docker run` argument construction in `run.rs`.

---

## Docker Integration

### Why CLI, Not API

`dcc run` and `dcc exec` require transparent interactive TTY pass-through.
Achieving this via the `bollard` API requires explicit TTY multiplexing and
`SIGWINCH` relay. Shelling out to `docker run -it` and `docker exec -it` gives
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

Every `dcc build` generates a Dockerfile and runs `docker build` over an
in-memory tar context, so every dcc-built image carries a `dcc.version` label:

```
docker build [--no-cache] [--pull] [--label devcontainer.metadata=<json>] [--label dcc.seed=<json>] --tag <image-tag> -
```

`--pull` is passed when `--no-cache` is given and the Dockerfile's `FROM` is an
upstream base image, so the base image tag is re-resolved upstream rather than
reusing a stale local image. For official `build` profiles this applies to the
user Dockerfile build; the generated dcc stage uses the local intermediate base
tag and does not ask Docker to pull it. The `-` argument instructs Docker to read
the entire build context (including the Dockerfile) from stdin as a tar archive.
No Dockerfile is written to disk.

For an image-only profile with `containerUser: root` and no features, env, ports,
or hooks, the generated Dockerfile is just `FROM <image>` plus the version
`LABEL` — one cached layer over the base image.

Official `build` sources are built first as a generated base image using their
Dockerfile, context, build args, and optional target. `dcc` then builds its own
generated stage from that base image so user creation, Features, baked supervisor
assets, and metadata labels remain consistent.

When features contribute runtime properties (mounts, commands, state, hooks, or
unsafe runtime settings), `dcc build` passes
`--label devcontainer.metadata=<json>` to `docker build`. The label value is a
JSON array with one entry per contributing feature and is stored inside the
image. `dcc run` and build preparation read it back via `docker image inspect`
rather than relying on any local file, making the image self-describing and
portable across machines. When the merged Feature + project state is non-empty,
`dcc build` additionally passes `--label dcc.seed=<json>` carrying the resolved
seed manifest (see State Seeding above).

After the image exists, `dcc build` resolves declared state paths against the
image environment, **hydrates** them from the image (see State Seeding), then
starts a temporary build-preparation container with workspace, cache, and
declared state mounts attached. It runs `onCreateCommand`,
`updateContentCommand`, and `postCreateCommand` in order; Feature hooks run
before the project hook for each phase. `dcc build --refresh-only` skips the
image rebuild and `onCreateCommand`, requires the profile image to exist, and
runs only `updateContentCommand` and `postCreateCommand`.

### Runtime Commands

Runtime entrypoints share the launch planner in `exec.rs`:

- `dcc start` starts or promotes a durable profile container and returns.
- `dcc run <name>` resolves a project or Feature command and executes it in the profile
  container.
- `dcc exec <cmd...>` executes an explicit command in the profile container.
- `dcc attach [cmd...]` runs attach hooks, then an explicit command or a shell-oriented
  default.

`dcc run`, `dcc exec`, and `dcc attach` reuse an existing profile container when one is
running. If no container is running, they start a one-shot container unless `--keep` /
`-k` is supplied. One-shot containers are stopped automatically when the last active
`dcc`-launched command finishes: the in-container supervisor (PID 1) drains its
active-command set and exits, and Docker's `--rm` removes the container. `dcc start`,
`dcc run -k`, `dcc exec -k`, and `dcc attach -k` set durable mode, which prevents
automatic teardown until `dcc stop`.

Lifecycle state — durable/one-shot mode, the active-command set, and the stopping flag —
is owned by the PID 1 supervisor and held in a container-private tmpfs at `/run/dcc`
(never host-backed). Docker labels (`dcc.container_id=<container-id>`) are used only for
stable container lookup. The supervisor scripts (`dcc-supervisor`, `dcc-ctl`,
`dcc-exec`) are generated from a single Rust source of truth (`src/supervisor.rs`) and
**baked into the image** at `/usr/local/share/dcc/` via the build context (decision 0004).
Every dcc-built image carries them, version-stamped by the `dcc.version` label; the CLI
refuses to drive an image whose major or minor version differs (patch drift is
compatible). Startup hook scripts are **not** baked — they are host-generated per launch
into `<workspace>/.dcc/<profile>.rt/start-hooks/`, bind-mounted read-only at
`/usr/local/share/dcc/rt`, and passed via `--start-hooks`, because `postStartCommand` may
contain `${localEnv:VAR}` which is only resolvable at run time.

**Phase 1 — pre-flight checks and argument construction**

Before starting or reusing Docker containers, the runtime planner:

1. Calls `docker image inspect` on the image tag to read its
   `devcontainer.metadata` label, if present. The label JSON is parsed into a
   `FeatureRuntimeConfig` (mounts, command, remoteEnv). A missing label is
   treated as no feature runtime contributions; a malformed label is a fatal
   error. It also reads the image's `Config.Env` (`{{json .Config.Env}}`, via
   `docker::inspect_image_env`) to resolve `${containerEnv:VAR}` references in the
   runtime properties.
2. Resolves `workspaceFolder`, `runArgs`, mounts, and state paths against the image
   environment when they contain `${containerEnv:...}`.
3. Rejects unsafe devcontainer runtime settings, unsupported or unsafe `runArgs`, and
   sensitive host mounts unless `--allow-unsafe-runtime` is present.
4. Resolves runtime state paths and prepares their profile-local host sources.
5. Calls `fs::create_dir_all` for any bind mount whose `src=` path falls under
   the host cache directory. Docker requires bind mount source paths to exist on
   the host before the container starts.

**Phase 2 — detached container start**

When no matching profile container is already running, `dcc` starts the container with
`-dit` (detached, interactive, TTY pre-allocated):

```
docker run
  --name <container-name>
  --label dcc.container_id=<container-id>
  --label devcontainer.local_folder=<workspace-root>
  --label devcontainer.config_file=<config-path>
  --rm
  -dit
  --workdir <workspaceFolder>  (default: /workspace)
  --memory <memory>            (default: 4g)
  --cpus <cpus>                (default: 2)
  <safe runArgs...>
  <allowed unsafe runtime args...>
  -u <containerUser>
  -e KEY=VALUE ...           (remoteEnv after variable substitution)
  -e KEY=VALUE ...           (feature remoteEnv after template substitution)
  --mount <spec> ...         (mounts after variable substitution)
  -v <workspace-root>:/workspace
  -v <host-cache-path>:/cache
  --mount <host>/.dcc/<profile>.rt:/usr/local/share/dcc/rt:ro  (startup hooks only)
  --tmpfs /workspace/.dcc
  --tmpfs /run/dcc:mode=1777  (supervisor lifecycle state; container-private)
  --entrypoint /usr/local/share/dcc/dcc-supervisor  (baked into the image)
  <image-tag> --mode oneshot|durable [--expect-command] [--start-hooks <rt>/start-hooks]
```

The container's PID 1 is the `dcc` lifecycle supervisor, a POSIX `sh` script baked into
the image at `/usr/local/share/dcc/dcc-supervisor`. It owns the durable/one-shot mode, startup
sequencing, `postStartCommand` hook execution, a readiness handshake, the
active-command set, and the teardown decision. This is deliberate: making a user
command PID 1 and attaching to it fails for commands that exit quickly (e.g. `ls`)
because the container can disappear before readiness polling or attach observes it.
User commands run via `docker exec` through the `dcc-exec` wrapper (phase 4), which
registers each command with the supervisor and waits for the readiness signal before
running it. `dcc` polls `docker inspect` at 100 ms intervals (up to 10 s) until the
container reports as running (`wait_for_running`); this detects total launch failure
(bad image, invalid mount) but is not a readiness check — readiness is the
supervisor's `bootstrap-status` handshake (phase 4).

`initializeCommand` is parsed for devcontainer compatibility but is not executed,
because `dcc` does not run devcontainer-defined commands on the host. When a new
container starts, `postStartCommand` runs **inside the supervisor** (PID 1), not via
a host-side `docker exec`. The host pre-substitutes each hook (resolving
`${localEnv:…}`, `${containerEnv:…}`, etc. using the image's baked environment) into
an executable script in `.dcc/<profile>.rt/start-hooks/`, named `NN-<source>` so
lexical order is execution order; feature hooks run before the project hook. The
directory is passed to the supervisor via `--start-hooks`. When it finishes, the
supervisor writes `/run/dcc/bootstrap-status` (`0` on success, `<exit-code>
<hook-name>` on failure) and signals waiters. There is no time-based startup grace:
`STARTUP_GRACE_SECS`, `PRIMED`, and the grace branch are deleted; a one-shot
`--expect-command` flag and a 10 s post-bootstrap orphan reaper (one-shot only)
cover the host-side gap between `docker run` and `docker exec`. Build-preparation
hooks (`onCreateCommand`, `updateContentCommand`, `postCreateCommand`) are not part
of ordinary runtime commands; `dcc build` owns them.

`runArgs` are deliberately allowlisted. Safe value-taking flags such as `--add-host`,
`--dns`, `--hostname`, `--label`, `--tmpfs`, `--shm-size`, `--ulimit`, `--platform`,
`--cap-drop`, and explicit `--env KEY=VALUE` are passed through. Privileged or
host-integrating flags (`--privileged`, `--cap-add`, `--security-opt`, `--pid=host`,
`--ipc=host`, `--network=host`, `--device`, and sensitive mounts/volumes) require
`--allow-unsafe-runtime`. Unknown flags are rejected. Top-level `privileged`, `capAdd`,
and `securityOpt` use the same explicit unsafe gate.

`workspaceMount` is parsed but ignored because `dcc` owns the project mount. `overrideCommand`
is parsed but ignored because `dcc` owns PID 1 (the lifecycle supervisor). `portsAttributes` and
`otherPortsAttributes` are parsed for compatibility; browser/preview auto-open behavior is
not implemented.

Note that `forwardPorts` no longer translates to `-p` flags. Publishing ports
with Docker's `-p` mechanism routes traffic through the Docker bridge network,
so the container application sees connections as coming from the bridge gateway
IP rather than `127.0.0.1`. Port forwarding is handled separately in phase 3.

**Phase 3 — port forwarding**

For each port in `forwardPorts`, `dcc run`, `dcc exec`, and `dcc attach` bind a `TcpListener` on
`127.0.0.1:<port>` on the host and spawns a Tokio task (see Port Forwarding
below). The listeners are bound before the foreground command runs so that ports are
ready as soon as the session begins. `dcc start` currently starts only the durable
container; it does not leave a background host-side port-forwarding process behind.

**Phase 4 — foreground command**

```
docker exec -i [-t] -u <containerUser> -w <workspaceFolder> <container-name> \
  /usr/local/share/dcc/dcc-exec <command...>
```

The foreground command runs via `docker exec` through the `dcc-exec` wrapper, with
stdio inherited, so output streams live and the command's real exit code is returned.
This works uniformly for one-off commands (`ls`) and interactive shells (`bash`). `-t`
is requested only when dcc's own stdin is a terminal, so non-interactive use (pipes,
CI) still works. The exit status is propagated via `std::process::exit`.

The `dcc-exec` wrapper first **registers** its command with the supervisor (writing a
record to `/run/dcc/active/<id>`) and then **waits for readiness** by running
`dcc-ctl wait-ready`. The wait is event-driven: `wait-ready` creates a per-waiter FIFO
in `/run/dcc/waiters/<id>` *before* checking `bootstrap-status`, so a signal cannot be
lost; if the status file already exists (steady state) it returns immediately. When
the supervisor finishes bootstrap it writes `bootstrap-status` atomically and signals
every waiter FIFO via a non-blocking `<>` open (so an orphaned FIFO with no reader
cannot wedge PID 1). On success `wait-ready` exits `0`; on hook failure it prints the
failing hook name and the tail of `/run/dcc/hook.log` to stderr and exits `252`, which
the host maps to a clear error. Only after readiness does `dcc-exec` run the user
command and deregister on exit. `dcc start` (which runs no command) calls
`dcc-ctl wait-ready` host-side so a hook failure surfaces immediately.

`dcc attach` first runs `postAttachCommand` hooks **host-side**, with Feature hooks
before the project hook. On a cold start, the host first waits for the supervisor's
readiness signal (`wait-ready`) so `postStartCommand` always completes before
`postAttachCommand`. With no explicit attach command, it executes:

```
/bin/sh -lc 'if [ -n "${SHELL:-}" ] && [ "${SHELL#/}" != "$SHELL" ] && [ -x "$SHELL" ]; then exec "$SHELL"; elif [ -x /bin/bash ]; then exec /bin/bash; else exec /bin/sh; fi'
```

**Phase 5 — teardown**

After the foreground command returns, all relay task handles for that invocation are
aborted. The `dcc-exec` wrapper deregisters the command from the supervisor (via an
`EXIT` trap) as the command exits. The supervisor's drain decision keys off an
in-process `arrived` shell variable (set to `1` the moment any command ever registers),
the active-command count `n`, and — for one-shot containers only — a 10 s orphan
reaper that starts after `bootstrap-status` is written. The rules are: if `stopping`
exists and `n == 0`, exit (drain); if one-shot, `arrived == 1`, and `n == 0`, exit
(normal one-shot teardown); if one-shot, `arrived == 0`, and 10 s have elapsed since
`bootstrap-status`, exit (orphan reaper — covers the host being killed before
registering); durable containers without `stopping` stay alive. There is no time-based
startup grace: `STARTUP_GRACE_SECS`, `PRIMED`, the `started` variable, and the grace
branch are gone — the 10 s window starts only after bootstrap completes, so a long
`postStartCommand` never brings it closer. Durable containers never reap, so `dcc
start` (which runs no command) and the build-preparation container are unaffected.
Docker's `--rm` removes the container once the supervisor exits. The host does not
manage teardown state — it only waits for the container to disappear so port
listeners are cleanly gone. Durable containers remain running until `dcc stop`.

`dcc run` command resolution is owned by `run.rs`: project commands are addressed as
`:<name>`, Feature commands as `<feature-id>:<name>`, and unqualified names are accepted
only when exactly one source defines them.

The `--tmpfs /workspace/.dcc` mount places an empty tmpfs at that path inside
the container, hiding the host `.dcc/` directory from the container.

### dcc stop

`dcc stop` has three variants:

- **`dcc stop`** (default, graceful): signals the supervisor via `docker exec
  dcc-ctl stop` to stop accepting new commands and exit after all remaining
  commands finish (drain). The host then waits for the container to disappear.
- **`dcc stop --now`**: signals `docker exec dcc-ctl stop-now`, which force-terminates
  running commands (`TERM`), runs shutdown hooks, and exits.
- **`dcc stop --kill`**: unconditionally `docker kill`s the container. Emergency path
  for wedged or corrupted containers.

If no container is running, all three variants are idempotent successes. If the
supervisor is unreachable during a graceful stop (e.g. the container is wedged), `dcc
stop` falls back to `docker stop` and points the user at `--kill`.

---

## Port Forwarding

The devcontainer spec distinguishes between *publishing* and *forwarding* ports.
Docker's `-p HOST:CONTAINER` flag *publishes* a port: traffic arrives at the
container via the Docker bridge network and the application sees the source
address as the bridge gateway (e.g. `172.17.0.1`), not `127.0.0.1`. An
application that binds only to `localhost` rejects such connections.

*Forwarding* means routing traffic through the container's own loopback
interface so the application sees the source address as `127.0.0.1`. `dcc`
implements this using a host-side TCP relay (`forward.rs`) and a baked connector
wrapper running inside the container.

### Relay architecture (`forward.rs`)

For each port in `forwardPorts`, `dcc run` requires a `TcpListener` on
`127.0.0.1:<port>` and also binds `[::1]:<port>` when IPv6 loopback is
available. IPv6 bind failure degrades explicitly to IPv4-only forwarding. All
requested listeners are acquired before relay tasks start, so a later bind
collision releases earlier listeners without leaving background tasks.

Each listener owns a long-running Tokio task. For each accepted connection the
listener retains a short-lived connection-handler in a task set and immediately
resumes accepting. Shutdown aborts and joins the listener tasks; dropping each
task set cancels its active connectors, so no detached relay survives the
foreground `dcc` command.

`handle_connection` opens a tunnel by spawning:

```
docker exec -i <container-name> /usr/local/share/dcc/dcc-connect 127.0.0.1 <port>
```

`dcc-connect` selects a known compatible TCP client and connects to
`127.0.0.1:<port>` on the container's own loopback interface. `docker exec -i`
pipes the process's stdin/stdout back to the host. The handler copies both
directions concurrently:

```
host TCP socket  ←→  docker exec -i dcc-connect  ←→  app (127.0.0.1:<port> inside container)
```

Because `nc` connects from within the container, the application sees the
connection as originating from `127.0.0.1`, not from the Docker bridge.

When client input reaches EOF, the relay flushes and drops the child stdin pipe
immediately so `docker exec` can observe EOF while stdout continues to drain.
The wrapper then half-closes the application-facing socket. This allows a client
to finish a request with a write half-close and still receive the complete
response. Once both directions finish (or either copy fails), the connector is
reaped and the handler task exits.

### Connector compatibility

Netcat command-line and EOF behavior are not uniform. OpenBSD netcat needs `-N`
to half-close its network socket after stdin EOF; Nmap Ncat performs the needed
half-close by default and does not accept OpenBSD's short flag; BusyBox and
traditional netcat do not expose the required interface.

The POSIX `dcc-connect` wrapper is baked at `/usr/local/share/dcc/dcc-connect`
and owns this variant boundary. Its selection order is:

1. `nc.openbsd -N HOST PORT`;
2. an executable identifying itself as Nmap `ncat`, invoked without `-N`;
3. generic `nc -N HOST PORT`, only when `nc -h` advertises standalone `-N`;
4. a clear unsupported-connector error.

The wrapper also provides `--check`, validates its fixed loopback host and port,
and executes a selected program with direct arguments rather than shell
evaluation.

For non-empty `forwardPorts`, the generated Dockerfile copies the wrapper after
Feature installation and runs `dcc-connect --check`. A compatible client from
the base image or a Feature requires no installation. Otherwise the build tries
the following packages and runs `--check` again:

| Package manager | Package installed |
|---|---|
| `apt-get` (Debian/Ubuntu) | `netcat-openbsd` |
| `apk` (Alpine) | `netcat-openbsd` |
| `yum` (RHEL/CentOS) | `nmap-ncat` |
| `dnf` (Fedora/RHEL 8+) | `nmap-ncat` |

An arbitrary pre-existing `nc` no longer short-circuits provisioning. Unsupported
BusyBox or traditional variants therefore lead to installation of a compatible
client when a supported package manager is available, or a build-time error.
If future variants make help-text probing too fragile, a small compiled connector
is the bounded fallback; the fixed host-side executable boundary would not change.

### Handle lifetime and cleanup

Each listener has an explicit shutdown channel and retained `JoinHandle`.
Connections are retained in that listener's `JoinSet`; shutdown stops
acceptance, aborts active handlers, and joins them before returning. Connector
processes use `kill_on_drop` and are explicitly killed and waited on after a
relay result, so cancellation cannot detach a subprocess.

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
- `customizations.dcc.commands` → stored in the feature's label entry for `dcc run` command resolution; legacy top-level `scripts` is normalized with a warning
- `customizations.dcc.state` → fixed container path variables are substituted before validation and storage in the feature's label entry; `${containerEnv:...}` remains deferred; mounted before project state at runtime
- unsafe runtime properties → rejected unless `--allow-unsafe-runtime` is present, then stored in the feature's label entry

Features that contribute at least one runtime property get an entry in the
`devcontainer.metadata` label JSON array. Features that contribute only build-time
properties (`containerEnv`, `options`) are omitted from the label. The label is
embedded in the image via `docker build --label`.

### Supported feature properties

| Property | Description |
|---|---|
| `options` | Configuration options. Keys are uppercased and passed as environment variables to `install.sh`. |
| `containerEnv` | Environment variables baked into the image as Dockerfile `ENV` directives. Only container-side variables are substituted. |
| `remoteEnv` | Environment variables passed as `-e` runtime flags to `docker run`. Stored as raw templates; substituted at `dcc run` time. |
| `mounts` | Additional mounts attached at `dcc run` time. |
| `customizations.dcc.commands` | Named feature commands; legacy top-level `scripts` is accepted with a warning. |
| `customizations.dcc.state` | Feature-contributed persistent state paths. |
| `installsAfter` | Soft ordering hint. Feature IDs that this feature should be installed after (if present). |
| `dependsOn` | Hard dependencies. Missing dependencies are added to the installation set automatically. |
| `init`, `entrypoint` | Parsed and warned as ignored because `dcc` owns PID 1 startup. |
| `privileged`, `capAdd`, `securityOpt` | Unsafe runtime settings gated by `--allow-unsafe-runtime`. |

Feature `containerUser` and `remoteUser` are rejected.

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
   in `library_scripts/`). Safe archive-root directory markers such as `./` are
   ignored. Absolute paths, parent traversal, links, special entries, and empty
   non-directory paths are rejected before content enters the build context. A
   missing `devcontainer-feature.json` remains an install-only compatibility mode;
   supplied metadata must be valid JSON with valid field types.

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
# Only present when updateRemoteUserUID is enabled (default) and containerUser
# is a non-root named user on a Linux or macOS host:
ARG REMOTE_USER='<user>'
ARG NEW_UID=<host-uid>
ARG NEW_GID=<host-gid>
RUN <updateRemoteUserUID remap: sed-rewrite /etc/passwd + /etc/group,
     chown -R the user's home, with reference no-op conditions>
# Baked dcc runtime assets:
COPY .dcc-generated/ /usr/local/share/dcc/
RUN chmod +x /usr/local/share/dcc/dcc-*
# Only present when forwardPorts is non-empty (see Port Forwarding below):
RUN ( /usr/local/share/dcc/dcc-connect --check >/dev/null 2>&1 \
 || (command -v apt-get >/dev/null 2>&1 && apt-get update -qq && apt-get install -y --no-install-recommends netcat-openbsd) \
 || (command -v apk     >/dev/null 2>&1 && apk add --no-cache netcat-openbsd) \
 || (command -v yum     >/dev/null 2>&1 && yum install -y nmap-ncat) \
 || (command -v dnf     >/dev/null 2>&1 && dnf install -y nmap-ncat) ) \
 && /usr/local/share/dcc/dcc-connect --check
```

The devcontainer.json `containerEnv` `ENV` directives appear immediately after
`FROM`, before any feature blocks. Feature `containerEnv` `ENV` directives are
emitted immediately before each feature's `RUN` step so the variables are
available to `install.sh` and remain set in the image for all subsequent layers.

The user-creation step is idempotent (`id` short-circuits when the user already
exists) and cross-distro compatible: `useradd` covers Debian/Ubuntu/RHEL/Fedora;
`adduser -D` covers Alpine/BusyBox. It runs after features so a feature that
already creates the user does not cause a conflict.

The `updateRemoteUserUID` remap (`src/uid.rs`) follows the reference
`devcontainers/cli` `scripts/updateUID.Dockerfile` logic and runs immediately
after user creation so features install into the already-remapped home
ownership. It is planned by `plan_uid_remap` from the resolved `containerUser`,
the `updateRemoteUserUID` config flag (default `true`), and the host process
uid/gid (`host_ids`, captured via `id -u`/`id -g` on Linux and macOS). Windows
and unsupported hosts return no host IDs and explicitly skip remap planning.
The remap `RUN` and its `ARG`s are emitted by `remap_dockerfile_block`; the
`--build-arg` values come from `remap_build_args` and are passed to
`docker build`. It no-ops (and emits a recognizable echo line) when the user is
not found in `/etc/passwd`, the uid/gid already match, or another user already
occupies the target uid (collision — it refuses to stomp); when a group already
occupies the target gid it keeps the old gid and still updates the uid. A
`containerUser: root` profile is unaffected: the remap skips root, so no remap
`RUN` is ever emitted for root profiles.

Because the remap is baked into the image at build time, state hydration
(`src/seed.rs`) sees the remapped `/etc/passwd`. Hydration still re-owns copied
state for non-root `containerUser` profiles, because a Dockerfile can create and
`chown` state paths before the remap step, leaving those files with the user's
old numeric uid. This keeps declared state writable without reordering T-0022's
hydration.

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
dcc [--strict] [--dry-run] [--debug] [--format text|json] [-p/--profile <name>] <command> [global-flags] [command-flags] [--] [args...]

Commands:
  build  [--no-cache] [--refresh-only] [--reseed-state]
  run    [--memory <size>] [--cpus <n>] [command-name]
  exec   [--memory <size>] [--cpus <n>] <command...>
  start  [--memory <size>] [--cpus <n>]
  attach [--memory <size>] [--cpus <n>] [command...]
  stop
  id
  profile list
  feature [--add <reference>] [--remove <reference>]
```

`--profile` (`-p`), `--strict`, `--dry-run`, `--debug`, and `--format` are clap
**global arguments** declared once on `Cli` and read from the single global fields. As
global arguments they are accepted in both positions — `dcc -p claude --strict run`
and `dcc run -p claude --strict` are equivalent — so users are not forced to
remember whether the flags precede or follow the subcommand. Earlier versions
declared `-p` on each subcommand to allow it after the subcommand; the global
argument supersedes that, supporting both orderings with no duplication.
`--profile` defaults to `"devcontainer"`. `--strict` affects config parsing,
which applies identically across all subcommands. For commands like `dcc exec` and
`dcc attach`, whose trailing arguments form the in-container command, global flags
must precede the first positional argument, otherwise they are passed through to
that command.

Implemented with `clap` derive macros. `dcc run` accepts an optional named command
from project or Feature metadata; explicit argv execution belongs to `dcc exec`.

`--dry-run` validates the command to the Docker boundary and exits before Docker
subprocesses, cache/state preparation, port forwarding, or lifecycle hook execution.
The text report is intentionally short; `--format json` emits a stable report with
the command, profile, resolved container id, config path, `docker_invoked: false`,
checks performed, and Docker-dependent checks skipped. The explicit container id lets
callers verify that path-based profiles target the same identity across build, runtime,
and stop planning without invoking Docker. Dry-run cannot validate image-derived data
such as Feature runtime metadata from `devcontainer.metadata` or
`${containerEnv:...}` values that require image/user probing.

`--debug` emits command-specific resolved details to stderr. Runtime commands print
the launch plan and Docker command. `build`, `stop`, and `id` print the resolved
profile/config/container/image details available before acting.

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
- `--dry-run` validates command/config behavior before Docker invocation
- `--format json` emits parseable dry-run reports
- `dcc run --` is accepted syntactically, while direct command execution is tested
  through `dcc exec`
- `tests/docker_boundary.rs` places an argv-recording fake `docker` first on `PATH`
  and drives the compiled CLI through version gates, build pull policy, and runtime
  resource plumbing; this complements, rather than replaces, live Docker smokes

Integration tests that require a live Docker daemon are annotated `#[ignore]`.
