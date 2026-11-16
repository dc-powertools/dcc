# T-0108: Separate Persistence from Image-Controlled Installations

## Identity and source

- Revision: r1, 2026-11-16.
- Authority: user requested recording an implementation task in dcc, preserving the
  audit findings, requiring coordinated Feature work, and leaving inclusion/exclusion
  specification as open design work.
- Catalog: [task catalog](README.md), which owns lifecycle and next action.
- Dependency/evidence: T-0107 [Feature state audit](../quality/0107-feature-state-audit.md).
- This change captures future work only; do not start design or implementation as
  part of task intake. Primary task remains unchanged.

## Goal

Make persistence explicit and independent of image installation contents. Durable
user data and caches survive container recreation and image rebuilds without
content hashing or automatic image refresh. Image-controlled binaries, packages,
toolchains and their installation metadata come from the image build; durable
upgrades require rebuilding the image. Installs or updates inside a running
container may work, but their image-controlled contents remain ephemeral and are
lost on recreation. This does not require making the container filesystem read-only.

Replace the current broad state-seeding/digest/reconciliation approach with an
ownership-based contract implemented coherently in dcc and the Features repository.

## Motivation and evidence

Current `dcc build` hashes every declared state path to decide whether image seeding
can refresh it without overwriting user changes. `src/seed.rs` constructs an entire
tar archive in memory before hashing. Large durable paths such as node_modules can
reach tens of GB, making this unsuitable. Streaming would reduce memory but would
still scan all content; it does not address the underlying ownership problem.

Measured debug-binary builds:

- [T-0105](../quality/0105-build-memory.md): 965.7 MiB existing Codex state caused
  988.4 MiB dcc RSS; plain Debian baseline was 15.4 MiB.
- [T-0106](../quality/0106-real-profile-memory.md): fresh real-world profile state
  of 470.9 MiB caused 489.2 MiB peak dcc RSS after image export.

## Coordinated repository scope

In dcc: design metadata/configuration and validation, persistent mount/file handling,
image visibility, lifecycle behavior, migration of existing durable state and seed
ledgers, removal/replacement of content-hash reconciliation, tests and documentation.
Review project-declared state as well as Feature-contributed state. Account for
existing image/controller compatibility and schema/version requirements.

In the Features repository: review each package's actual install and runtime layout,
identify exactly which files/directories should persist, exclude image-controlled
installations and ephemeral scratch, and update metadata/install/startup scripts,
versions, tests and documentation as needed. Coordinate dcc support and Feature
rollout so existing installations remain usable. Audit all packages, including
those with no direct state declaration, rather than treating the preliminary map as
an exhaustive or permanently stable upstream contract.

## Findings to carry forward

`~` below denotes container HOME; workspace denotes containerWorkspaceFolder.
Six Features currently declare eight durable paths:

| Feature | Current durable path | Ownership findings |
| --- | --- | --- |
| aws-cli | ~/.aws | Persist user config, credentials and SSO/CLI caches. Executable lives outside it at /usr/local/aws-cli. Default template at /usr/local/share/aws-cli/config is copied on startup only when user config is absent. |
| claude | ~/.claude | Persist user settings/auth/session/project/skill/plugin state. downloads/ is installer staging and should not be durable. Executable versions already live outside it under ~/.local/share/claude/versions with ~/.local/bin/claude launcher. Audit ~/.claude.json outside current state root for config/auth coverage. Installer-generated defaults/backups are not automatically image-owned executable payload. |
| codex | ~/.codex | packages/standalone and packages/app-server-daemon, including releases/current links/selection/update/install metadata, belong to image-controlled installation storage. Preserve user config/auth, sessions/history, user skills/plugins, memories and state databases, including necessary SQLite WAL/SHM companions. tmp/ and app-server-control/ plus sockets/PIDs/locks in app-server-daemon/ are generally ephemeral. Distinguish runtime app-server-daemon/ from packages/app-server-daemon/. |
| mo | ~/.mo | Feature and Homebrew formula install mo under /home/linuxbrew/.linuxbrew, outside MO_HOME. Persist application state, but inspect runtime internals before committing to a complete inclusion/exclusion map; runtime helpers/caches have not been exhaustively audited. |
| node | /workspace/node_modules | Entire dependency directory is simple persistence; Feature installs Node/npm under /usr/local. Startup npm install manages project dependencies; package-manager operations may replace content. No image seed refresh or hashing required. |
| rust | ~/.rustup | Treat installed toolchains and selection/install metadata as image-controlled under this policy. Do not blindly persist settings.toml that can select a toolchain absent from a rebuilt image. Durable user overrides, if supported, require explicit separate treatment. |
| rust | ~/.cargo | Image-controlled rustup/proxy executables in bin/ and generated env integration; persist registry/, git/, configuration, credentials and required cache bookkeeping. bin/ can mix image tools and later cargo-install tools: under the requested policy runtime executable installs and matching .crates.toml/.crates2.json stay ephemeral; decide whether installation-root separation is needed. |
| rust | /workspace/target | Entire build artifact directory is simple persistence. |

Git, sudo, linux-package and Playwright have no directly declared state. Playwright
inherits node_modules through Node. Its OS dependency record must track the same
container/image as OS libraries; do not persist that record across recreation.

User-selected skills/plugins can contain code yet still be user state; classification
must follow ownership rather than treating every executable/code file as image-owned.
Codex daemon packages can be provisioned at runtime today, so verify daemon bootstrap,
update and launch behavior with ephemeral package storage or image provisioning.
CODEX_INSTALL_DIR only changes launcher placement; it does not independently move
the package tree away from CODEX_HOME.

## Open design work — deliberately unresolved

Determine the best way for each Feature package to specify persistent **directories
and files**, including inclusions and exclusions. No syntax, whitelist/blacklist
strategy, glob semantics, new API, or mount technique is selected by this task.

Evaluate at least:

- Exact file/directory selections versus broader roots with exclusions; unknown new
  upstream paths and nested mixed-ownership trees.
- How excluded image paths remain visible/writable when their parent is persistent;
  symlinks, nested mounts, absent files and container-user ownership/permissions.
- Whether relocating installation roots simplifies particular packages, versus using
  selection/mount rules; preserve launchers and package-manager bookkeeping.
- Distinguish persistent data, image-provided installs, and ephemeral runtime scratch.
- Handling image-provided user configuration defaults, separately from tool installs:
  explicit initialize-if-absent or another deliberate policy without overwrite of
  existing user data and without a content scan of durable trees.
- Project/Feature configuration composition, substitutions, validation, overlaps,
  backward compatibility and safe migration from current state/seeding behavior.
- Safe handling of existing durable package trees, credentials/history, seed ledgers
  and previously built images; do not silently delete user content during migration.
- Coordinated release/version sequencing across both repositories and recovery when
  configurations or tools do not support the new contract.

## Acceptance criteria for future delivery

- [ ] Design resolves and documents inclusion/exclusion for files and directories,
  ownership semantics and migration before implementing the shared contract.
- [ ] dcc persists selected data without content-based seed refresh/reconciliation or
  whole-directory hashing/scanning proportional to durable data size during build.
- [ ] Image-controlled packages reflect rebuilt images after recreation; runtime-only
  upgrades do not become durable through persistent parent directories.
- [ ] Every Feature is audited and updated/tested as needed, with the preliminary
  ownership findings above validated against its supported upstream versions.
- [ ] Existing mutable settings, credentials, session data and caches survive rebuild
  and recreation; migration preserves user data and provides a recovery path.
- [ ] Defaults initialization and runtime scratch handling have explicit behavior.
- [ ] Targeted tests verify files, directories, mixed parents, exclusions, symlinks,
  ownership, image upgrades, concurrent sessions and relevant lifecycle hooks.
- [ ] Large node_modules/target-style data tests demonstrate bounded dcc memory and
  absence of content scanning, beyond merely replacing the tar buffer with streaming.
- [ ] End-to-end Codex/Claude/Rust and applicable package startup checks pass; required
  dcc checks and affected Feature checks pass, with schema/docs updated coherently.

## Risk and implementation sequence

Design first, then implement the shared dcc contract and coordinated Feature changes,
then validate migrations and end-to-end behavior. Plan concrete slices after design;
no child task IDs or publication actions are created by this capture.

Main risks are hiding image binaries behind persistent mounts, losing credentials or
history, retaining stale package selection metadata, daemon/bootstrap failures, and
incompatible dcc/Feature rollout. Treat data migration and the public configuration
contract as high-risk work when selecting verification and review gates.
