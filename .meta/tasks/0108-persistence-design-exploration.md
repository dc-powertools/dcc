# T-0108: Directory Persistence Design Exploration

2026-11-16, r3. Unified exclusion behavior agreed with the user; illustrative schema,
package layouts and remaining implementation details are not frozen or implemented.

## Agreed direction and proposed configuration

Prefer directory persistence. Select a tool's coherent data root, use supported
configuration variables to separate installed tools from data where possible, and
permit a short list of literal excluded child directories where a tool mixes them.
Avoid general include/exclude glob rules and individual mounts for mutable files.

Conceptual extension of existing state object syntax (not final schema):

```json
{
  "path": "${containerEnv:HOME}/.codex",
  "exclude": ["packages", "tmp", "app-server-control", "app-server-daemon"]
}
```

String directory entries remain the simple form. Each object selects the whole root.
Exclusions are literal relative directory paths, not patterns; reject absolute paths,
parent traversal, escaping symlinks and conflicting/overlapping selections. No rule
ordering, negation, file globs, wildcard expansion or re-inclusion semantics.
Excluded directories get fresh image-initialized writable storage for each newly
created container. Persisted roots start empty and never copy or refresh image
contents automatically. Excluded image directories that should start empty (scratch,
installer downloads) are cleaned/prepared by their Feature at build time. Defaults
initialization for user configuration is explicit initialize-if-absent logic.

One semantics covers installed and scratch directory exclusions: recreate from the
image, whose scratch directories are empty. Ownership metadata for mount destinations
must be set at build time or prepared before user hooks. No automatic copying of an
entire persistent root, no content digest and no recursive data-size scan.

## Clarification accepted in discussion (r3)

No separate scratch/staging mode is needed. Every excluded directory starts with
its image contents and remains local to that container. Feature installers must
leave scratch/staging empty and prepare appropriate ownership/permissions; tool
packages remain populated. The implementation sees an empty directory as the same
case with no files to copy, with at most a small fast path.

Observed image inventory:

| Proposed exclusion | Initial image contents |
| --- | --- |
| Codex packages/ | Populated standalone binaries/resources and install selection metadata. |
| Codex tmp/ | Installer helper links/locks; clean during Feature build. |
| Codex app-server-control/ | Absent; Feature should prepare empty runtime directory. |
| Codex app-server-daemon/ | Absent runtime-control directory; distinct from packages/app-server-daemon/. |
| Claude downloads/ | Empty staging directory. |

packages/app-server-daemon/ appeared only after Codex runtime use. Provisioning that
package during build versus ephemeral runtime bootstrap remains a package-level
choice under the same exclusion rule. Cleanup/ownership and populated-to-empty
image changes must be tested without adding a separate management path.

## Package mapping

| Feature | Provisional simplest layout |
| --- | --- |
| AWS | Persist whole ~/.aws. Keep CLI and default template outside; preserve initialize-if-absent behavior. |
| Claude | Persist whole CLAUDE_CONFIG_DIR (currently ~/.claude); exclude downloads/. Keep executable versions and launchers outside state. Set the variable consistently for runtime, and align build initialization or explicit defaults. |
| Codex | Persist whole CODEX_HOME, excluding packages/, tmp/, app-server-control/ and runtime app-server-daemon/. Keep all user files/SQLite sidecars together. Revisit .tmp plugin staging separately after runtime verification; do not blindly classify user-selected plugins/skills as image installation payload. |
| Mo | Persist whole MO_HOME; installed executable is in Homebrew. Add exclusions only after actual runtime helper/scratch layout is verified. |
| Node | Persist whole workspace node_modules. Feature installs Node/npm outside it. Project npm commands may add, replace or remove packages; dcc does not reconcile image seeds. |
| Rust | Keep RUSTUP_HOME and rustup/proxy install bin directory outside persistent CARGO_HOME. Persist whole CARGO_HOME and workspace target. Set CARGO_INSTALL_ROOT outside persistent data so runtime cargo-install tools and bookkeeping stay ephemeral. This avoids selecting individual config/credential files and avoids a bin exclusion at the persistent root. |
| Git/sudo/linux-package | No new state. |
| Playwright | Existing Node dependency supplies node_modules persistence; keep dependency-state record with ephemeral OS libraries. |

For Rust, example locations are /usr/local/share/rustup and
/usr/local/share/rust-tools for image-installed tools, ~/.cargo for persistent data,
and /usr/local/share/cargo-installed for default cargo-install output. These exact
paths are provisional; installer user permissions/PATH and runtime installs must be
tested. Cargo documents that CARGO_INSTALL_ROOT controls both bin/ and associated
.crates.toml/.crates2.json. Image build customizations installing tools should use
that same image-controlled install root. Explicit user --root overrides are deliberate
user choices; this is a default ownership policy, not a security boundary.

Changing Cargo install roots also needs to account for historical bin/ and .crates*
files in existing durable CARGO_HOME during migration so stale tools do not shadow
the image. Preserve user data; do not silently delete old installs.

## Why directory boundaries

A writable single-file bind mount is a mountpoint. Programs commonly write a sibling
temporary file then rename it over the destination; replacing a mounted file can fail
with EBUSY. Atomic save inside a mounted parent directory works. SQLite also needs
its database and associated sidecars/locking behavior together. Directory mounts
naturally support newly created files, changing schema-version filenames and future
data paths without a continuously maintained whitelist.

A symlink for every selected file is not a generic fix: some applications replace
links, reject them for credentials, or resolve atomic saves differently. Do not
promise transparent persistence for arbitrary isolated mutable files. Prefer the
application's supported config-directory override. Existing file-state declarations
need an explicit compatibility policy; this recommendation does not silently remove
existing support or claim all such files can be moved. If an unavoidable isolated
file remains, design/test its application's write behavior before selecting a method.

## Docker realization and Linux evidence

Docker cannot express a filter over one bind mount. A directory exclusion can be
implemented with an additional nested anonymous Docker volume at the excluded path:

```sh
--mount type=bind,src=<profile-state>/codex,dst=/home/dev/.codex
--mount type=volume,dst=/home/dev/.codex/packages
```

Docker copies the image's excluded directory contents into the new empty volume.
Each new container gets a new volume, initialized from its image, so no stale package
state is shared across recreation. The excluded volume is shared by all exec/attach
sessions within that container. Do not assign a stable per-profile persistent volume
name to the excluded package tree. Clean up with --rm or explicit docker rm -v, and
handle creation failures/orphans. Keep build-prep excluded volumes container-local
as well; do not accidentally promote their runtime-only upgrades into durable state.

This duplicates excluded image bytes into a fresh volume on container creation;
it is not zero-copy and adds disk/startup I/O proportional to installed payload,
not durable user data. Prefer separating install paths when supported. An ordinary
image directory cannot simply be recovered by mounting over its parent, and a host
bind source cannot name a path inside that same container's image. Read-only excluded
storage is not the default, because the user allows ephemeral runtime updates.

Small Docker 29.9.3 Linux ARM64 spikes in /tmp/dcc-persistence-design-spike:

- Persistent parent bind + anonymous child volume exposed image-v1, while parent
  image-default file was hidden. Parent user file survived between containers.
- Mutating excluded child in first container did not survive second container;
  second container again read image-v1.
- Image-v2 exposed image-v2 while keeping parent user data. A stale package file in
  host durable parent remained intact but hidden; runtime read current image-v2.
- Direct file bind: mv over destination failed with Device or resource busy.
  Same-directory atomic replacement inside directory bind succeeded.
- A transient spike setup attempt hit host ownership on Docker-created mountpoint;
  corrected ownership only in the spike fixture, then completed stale-data test.
  This reinforces need to prepare excluded directory ownership explicitly.
- All spike containers used --rm and were removed. Tiny fixture images/logs retained.

Claude 2.1.325 verification used disposable container, no network and no credentials,
HOME=/tmp/claude-design-home, CLAUDE_CONFIG_DIR=/tmp/claude-design-state,
DISABLE_AUTOUPDATER=1. An unauthenticated CLI invocation exited 1 with Not logged in,
as expected, and created .claude.json inside CLAUDE_CONFIG_DIR. Only filenames were
inspected. This resolves the prior uncertainty for that version: no extra mutable
file mount is needed for the root config when that override is set. Test equivalent
behavior for supported versions/auth flows before changing Feature guarantees.

## macOS/Linux storage considerations

Linux containers on Docker Desktop run in a Linux VM. Mounts declared at container
creation do not require host mount propagation, privileged setup or Linux overlayfs
on the macOS host. Docker documents that mount propagation does not work on Desktop;
do not base this solution on dynamically propagating mounts from the host.

The persistent-root backend is separate from selection rules:

- Existing profile-local host binds retain host visibility/backups and existing cache
  layout. macOS paths must be shared with Desktop. Bind sharing can have substantial
  metadata overhead for node_modules/target and needs SQLite/locking qualification.
- Docker-managed persistent volumes keep data on the VM Linux filesystem and are
  attractive for database/cache performance and filesystem fidelity on macOS. They
  change backup/inspection/migration/cache-location behavior. Do not silently switch
  the existing backend while specifying exclusions; decide that independently.
- Anonymous excluded volumes stay in Docker's Linux storage on either platform and
  are recreated from image on container recreation, independent of persistent backend.

No macOS/Desktop runner is available here. Nested mount and file behavior were
verified only on Linux; final support must be tested on Desktop with the supported
file-sharing backend, ownership, atomic saves, SQLite and container cleanup.

## Alternatives and remaining decisions

Explicit positive directory lists work for known separate roots (node_modules,
target) but root-level Codex auth/config/databases force many mutable file mounts or
custom relocation. A general glob language requires content discovery, missing-file
semantics, re-inclusion precedence and tracking newly created files: unnecessary here.
Per-file copy-in/copy-out adds synchronization/loss risks and concurrent attach issues.
Symlinking excluded Codex package directories changes upstream installer assumptions
(the installer explicitly avoids pruning symlinked package roots); prefer real mounts.

Resolve final schema/versioning and legacy file-state semantics, persistent backend,
excluded-storage cleanup/recovery, migration, defaults initialization, Mo/Codex runtime
cache layout and package manager integration before implementation. The unified exclusion behavior is agreed; remaining details are provisional and task
remains open. No public API or product implementation changed.

## Primary references

- https://docs.docker.com/engine/storage/bind-mounts/
- https://docs.docker.com/engine/storage/volumes/
- https://docs.docker.com/desktop/settings-and-maintenance/settings/
- https://doc.rust-lang.org/cargo/commands/cargo-install.html
- https://doc.rust-lang.org/cargo/reference/environment-variables.html
- https://code.claude.com/docs/en/settings
- https://github.com/openai/codex/blob/main/scripts/install/install.sh

Evidence/source and documentation whitespace review passed. User explicitly requested
recording the discussion and agreed approach in T-0108; implementation remains open.
