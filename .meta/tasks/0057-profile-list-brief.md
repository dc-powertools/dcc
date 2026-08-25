# `dcc profile list` Task Brief

## Identity And Source

- Task ID: T-0057
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User instruction
- Source reference and date: Product-owner request, 2026-08-24
- Parent or split task IDs: None

## Goal

Users can discover every named profile selectable from the workspace's direct
`.devcontainer/*.json` files through `dcc profile list`, with deterministic human and
machine-readable output.

## Background

`dcc` accepts a profile name through `-p/--profile`, but currently requires users to
inspect `.devcontainer` themselves to learn which names are available.

## Scope

In scope:

- Add the nested `dcc profile list` command.
- Discover direct, selectable `.json` profile files without loading their contents or
  invoking Docker; ignore filenames that cannot be represented by Clap's string profile
  argument.
- Sort output lexicographically by profile name.
- Print one profile per line in text mode, annotating `devcontainer` as the default.
- Escape control characters and backslashes in text names so untrusted filenames cannot
  inject terminal control sequences or extra records.
- Provide stable JSON entries containing `name`, workspace-relative `config`, and
  `default`.
- Document the command and cover discovery, ordering, filtering, empty results,
  structured output, debug output, and execution from a workspace subdirectory.

Out of scope:

- Discover path-based `-p` arguments outside `.devcontainer` or files in nested
  directories.
- Validate or merge profile configuration contents.
- Create, edit, select, build, or inspect Docker state for profiles.

## Users And Workflows

| User/Actor | Workflow | Expected Change |
| --- | --- | --- |
| Developer | Run `dcc profile list` from a workspace or subdirectory | Sees sorted selectable profile names without Docker. |
| Script or editor integration | Run `dcc profile list --format json` | Receives a stable object with ordered profile records. |

## Acceptance Criteria

- [x] `dcc profile list` succeeds without Docker and lists only direct regular or
  symlinked `.json` files whose names map to `-p <name>`.
- [x] Text output is sorted, one record per line, and marks `devcontainer` as
  `(default)`; an empty directory produces empty stdout.
- [x] JSON output is `{"profiles":[...]}` with ordered objects containing exactly
  `name`, `config`, and `default`; an empty directory produces an empty array.
- [x] Debug output identifies the command, workspace, and discovered count on stderr
  without changing stdout.
- [x] Help and user documentation explain discovery scope and output.

## Constraints

- Preserve all existing `-p/--profile` resolution behavior for other commands.
- Do not parse profile contents or add dependencies.
- Filesystem failures at the discovery boundary must retain useful path context.

## Workflow Route Rationale

- Cataloged route and risk: Initiative / Medium.
- Why this route: The change adds a nested public CLI surface, output contracts,
  filesystem discovery, tests, and documentation.
- Why this risk gate: Incorrect filtering or unstable output would break discovery and
  integrations, but the operation is read-only and isolated from Docker.
- Upstream artifacts required: Existing CLI dispatch, workspace discovery, profile-name
  mapping, output-format conventions, and documentation.
- Escalation trigger: Discovery semantics require recursive search, config parsing, or a
  change to the global profile selector.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Non-profile files are listed | Users invoke invalid names | Require exact `.json` suffix and a file/symlink-to-file at the direct directory level. |
| Output order varies by filesystem | Scripts and snapshots become unstable | Sort by profile name before rendering. |
| Listing accidentally resolves the selected global profile | Irrelevant path errors or config access | Dispatch profile listing before `-p` resolution. |
| Text and JSON drift | Confusing integrations | Render both from one typed discovery result and test exact outputs. |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| Available named profiles are the direct `.devcontainer/<name>.json` files. | High | Existing `ProfileName::config_path` maps names exactly this way. |
| Human text should remain simple while JSON owns structured integration data. | High | Existing CLI uses `--format json` for stable structured output. |

## Verification Plan

- Automated checks: Focused unit and CLI integration tests, `cargo fmt --check`,
  `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and
  `cargo build`.
- Manual checks: Help output, text output, JSON output, debug stderr, and diff review.
- Documentation checks: Search command lists and profile sections for aligned syntax
  and semantics.
- Baseline or counterfactual evidence for new regression/behavior tests: Before the
  implementation, `dcc profile list` is rejected by Clap because no `profile` command
  exists; the new CLI tests protect the public contract.

## Done When

- Every acceptance criterion passes, documentation and architecture are aligned, the
  task record is complete, and the task-scoped change is committed locally.
