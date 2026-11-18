# T-0092 Feature validation quality record

- Date: 2026-09-28
- Route / risk: Initiative / Medium
- Owner/reviewer: primary Codex session, focused diff and contract review

## Scope and readiness

User acceptance and verification methods were recorded in the task note before
implementation. The mount-sharing amendment was incorporated before parser work.
Implementation follows existing clap dispatch, Serde metadata types, static state
and dependency checks. Changes cover the offline validation command and its mount
runtime prerequisite. Publishing, Feature CI, Docker execution, and remote
acquisition are excluded. Rollback is a local commit revert; no data migration.

## Acceptance and verification

| Criterion | Evidence | Status |
| --- | --- | --- |
| Single Feature and direct-child collection, nonzero invalid/empty/unreadable inputs | CLI tests with executable-free PATH and no workspace, ordered multi-file diagnostics, missing/file/directory/broken-link inputs | Pass |
| Schema validity and actionable JSON locations | Missing/unknown/wrong types, option anyOf branches, malformed JSON line/column, text and JSON reports | Pass |
| Explicit extensions and separate upstream compliance | Identical mount/remoteEnv/scripts fixtures pass dcc and fail upstream; upstream accepts tool customizations without asserting dcc compatibility | Pass |
| Actual parser compatibility | Invalid bind source, dependency reference, and state path pass upstream but fail dcc with locations | Pass |
| One mount parser, readonly runtime effect | Shared parser tests, Feature build-context-to-label round trip compared with project parser, fake-Docker final `--mount` argv assertions | Pass |
| Editing compatibility | All 15 existing Feature editor tests pass; edit/validate mixing rejected | Pass |
| Final format/check/Clippy/full suite/build | `cargo fmt --check`, `cargo check --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` (592 unit + 114 integration passed; 38 ignored as designed), `cargo build --locked` | Pass |

Counterfactual evidence: the same extension inputs are rejected by the unextended
upstream schema; parser-incompatible inputs pass that schema and fail dcc. These
negative controls distinguish the validation layers. Tests also assert preserved
readonly at label and final Docker argv boundaries, so accepting but dropping the
extension cannot satisfy them. Existing object serialization regressions moved
into shared-parser table coverage, including anonymous volumes.

## Focused review

- Schema bytes and license copied from immutable upstream revision; checksum
  documented. Upstream-only does not alter schema semantics.
- Validator library's HTTP/file resolution features disabled; input metadata is
  never interpreted as a schema. Static command dispatch cannot execute scripts,
  initialize OCI, discover profiles, or invoke Docker.
- Extension replacements are narrow and version-controlled; unknown top-level
  properties and dcc customization properties remain rejected. Object readonly
  is rejected both by schema and by the runtime parser.
- Parser preserves string templates/options and object anonymous-volume behavior;
  runtime safety/authorization gates remain in place.
- Added regular-file guard after review found read could otherwise block on a FIFO.
- Added global-flag placement regression exposed clap's blanket
  args/subcommand conflict also rejecting global flags before validation. Replaced
  it with an explicit conflict limited to add/remove, preserving clap exit code 2.
- JSON Schema diagnostic messages mask instance values while naming constraints;
  no secrets, credentials, release/CI changes, or unrelated work were added.

## Batch and residual limits

One outcome spans validation and its explicitly required shared mount parser.
The larger diff includes the vendored upstream schema/license and dependency lock
entries. Keeping it together guarantees extensions cannot ship without runtime
support. Validation checks metadata and static parser compatibility; it cannot
prove scripts, remote dependencies, host paths, image variables, or Docker option
values work. Existing live Docker smokes remain intentionally ignored; deterministic
argv tests verify the requested runtime propagation without requiring Docker.

## Completion

Done. All required checks passed, including the corrected global-flag case and
all 12 validation CLI tests. Schema checksum, help output, documentation links,
diff, and focused security/contract review passed. No live Docker, remote Feature
acquisition, publishing, release, or external CI changes were required or performed.
