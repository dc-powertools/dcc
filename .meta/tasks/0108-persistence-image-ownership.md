# T-0108: Separate Persistence from Image-Controlled Installations

## Identity and source

- Revision: r4, 2026-11-16; user requested recording symlink relocation as the
  preferred approach and further Codex directory-structure investigation.
- Authority: user requested recording an implementation task in dcc, preserving the
  audit findings, requiring coordinated Feature work, and leaving inclusion/exclusion
  specification as open design work.
- Catalog: [task catalog](README.md), which owns lifecycle and next action.
- Dependency/evidence: T-0107 [Feature state audit](../quality/0107-feature-state-audit.md).
- Initial request captured future work only. The user subsequently requested design
  exploration of per-package selection and Docker Linux/macOS behavior. Product
  implementation is not yet started. Primary implementation pointer remains unchanged.

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
| codex | ~/.codex | packages/standalone and packages/app-server-daemon, including releases/current links/selection/update/install metadata, belong to image-controlled installation storage. Preserve user config/auth, sessions/history, user skills/plugins, memories and state databases, including necessary SQLite WAL/SHM companions. tmp/ and app-server-control/ require runtime validation. app-server-daemon/ mixes settings.json user preferences with process records: do not exclude it wholesale pending investigation. Distinguish it from installed packages/app-server-daemon/. |
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

## Agreed direction (r4)

Persist coherent data-directory roots, with a short list of literal excluded child
**directories** where installed software or scratch shares the same root. Prefer
supported installation/configuration-root separation where it simplifies a Feature.
Avoid a general glob/filter language and per-file mounts for mutable tool state.
Exact public metadata syntax remains to be designed; examples in the exploration
note are illustrative, not a frozen schema.

Use one behavior for all excluded directories: **initialize from that directory in
the selected image, keep changes local to the container, and discard those changes
when the container is replaced**. Feature installers must leave excluded directories
empty when empty is the correct initial state, and ensure correct permissions.
Installed payload and scratch/staging do not need distinct configuration types or
management paths. Handling an empty directory is the same initialization with no
files to copy, apart from any small implementation fast path.

Prefer relocating excluded contents to stable image-controlled locations outside
persistent roots, then creating/verifying symlinks at original paths after persistent
mounts are attached and before user hooks. The target uses the normal image/container
writable layer: no per-container payload-population copy, and updates remain ephemeral.
Installers leave appropriate targets empty for scratch, preserving unified semantics.
Perform relocation during the same installation layer to avoid image-byte duplication.
Qualify each package's symlink/updater/sandbox behavior; anonymous image-initialized
volumes remain a fallback candidate, not the preferred default.

See [exclusion alternatives investigation](0108-exclusion-alternatives.md) for Linux
Codex startup/daemon-bootstrap/recreation tests, installer pruning limitations,
migration requirements and rejected/limited Docker mounting alternatives.

In the inspected image, Codex packages/ contains the standalone installed payload.
Codex tmp/ held build-time helper links/locks that the Feature should clean;
app-server-control/ and runtime app-server-daemon/ were absent in that fresh image,
but this does not establish their future contents as disposable. Claude downloads/
was empty staging. Codex packages/app-server-daemon/ was created during runtime use,
not by the existing Feature image install. Decide whether to provision it during
build or let it bootstrap ephemerally under the same excluded packages/ directory.

## Remaining open design work

Resolve the final metadata schema, validation, existing isolated file-state support,
symlink preparation/lifecycle details, migration and per-package verification. In
particular, resolve Codex runtime app-server-daemon/settings.json persistence before
finalizing its excluded directory list. The directory-root
plus literal-directory-exclusion approach and unified initialization behavior above
are the agreed direction; do not reopen a separate scratch-versus-payload mode.


Evaluate at least:

- Literal relative-directory validation and overlap/symlink rules; unknown new
  upstream paths and nested mixed-ownership trees. No general glob or re-inclusion
  engine is proposed. Legacy file-state behavior still needs an explicit contract.
- How excluded image paths remain visible/writable when their parent is persistent;
  symlinks, nested mounts, absent files and container-user ownership/permissions.
- Whether relocating installation roots simplifies particular packages, versus using
  selection/mount rules; preserve launchers and package-manager bookkeeping.
- Distinguish ownership for Feature audits, but implement installs and scratch under
  the same image-initialized excluded-directory behavior.
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
- [ ] Symlink relocation, permissions, concurrent startup and migration are tested;
  package updater/pruning/sandbox compatibility is qualified or a fallback designed.
- [ ] Codex runtime directory investigation identifies and preserves user preferences,
  including app-server-daemon/settings.json, without persisting stale process state.
- [ ] All exclusions use the same image-initialization/container-lifetime behavior;
  both populated installed payload and empty scratch/staging cases are tested.
- [ ] Feature installers clean excluded scratch/staging directories and prepare their
  initial ownership/permissions; no separate scratch persistence mode is introduced.
- [ ] Defaults initialization has explicit behavior independent of durable-state
  content hashing.
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


## Design exploration checkpoint (r2)

User requested evaluating the clearest selection rules and Docker behavior on Linux
and macOS. See [directory persistence design exploration](0108-persistence-design-exploration.md)
for the provisional recommendation and Linux spikes. This is not an accepted final
schema or a completed implementation. Desktop qualification, metadata versioning,
backend/migration details and remaining package runtime verification stay open.


## Recorded discussion and evidence (r3)

See [directory persistence design exploration](0108-persistence-design-exploration.md)
for the package-by-package layout recommendation, Docker mount examples, Linux test
results and macOS limitations. User explicitly requested this task/documentation
update; recording the checkpoint does not mean the implementation task is complete.

Useful constraints from that discussion:

- Direct file bind mounts can reject atomic replacement (Linux spike returned EBUSY),
  while atomic save inside a mounted parent directory worked. Keep related SQLite
  data and sidecars together rather than mounting database filenames individually.
- Nested persistent-parent plus image-initialized excluded-child tests on Linux
  preserved parent user data, discarded runtime child changes on recreation, and
  selected image-v2 contents over a stale durable child without deleting old data.
- With CLAUDE_CONFIG_DIR set, Claude 2.1.325 created .claude.json inside that directory;
  a separate writable file mount was unnecessary. Verify supported versions/auth.
- Rust can separate RUSTUP_HOME/tool executables from persistent CARGO_HOME and use
  CARGO_INSTALL_ROOT to keep runtime-installed tools and their bookkeeping ephemeral.
- Persistent-root bind versus Docker-managed volume is an independent backend choice.
  Desktop file-sharing performance, permissions, atomic saves and SQLite need live
  qualification. Do not depend on host mount propagation on macOS.


## Investigation update (r4)

User selected symlink relocation as the preferred direction after exploring alternatives.
The [investigation record](0108-exclusion-alternatives.md) preserves the supporting
observations and limits. It supersedes the earlier anonymous-volume preference and
whole runtime app-server-daemon/ scratch classification, while keeping the unified
image-defined initial-state policy. Desktop validation, final schema, safe migration,
package compatibility and Codex directory mapping remain required before implementation.
