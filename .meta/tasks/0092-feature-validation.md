# T-0092: Offline Feature validation

## Outcome and scope

Implement `dcc feature validate <path>` for one Feature directory or direct-child
collection, with bundled pinned upstream schema, explicit dcc extensions, and an
upstream-only mode. Preserve add/remove. Runtime, validation, and project config
must share string/object mount parsing (user amendment, r2). Never accept ignored
object readonly fields. No Docker, scripts, builds, dependency resolution, publishing,
or changes to external Feature CI.

## Plan and acceptance

1. Share project/Feature mount parsing and prove readonly survives metadata labels
   and runtime argument construction.
2. Bundle upstream schema with source revision/license; compose explicit extensions
   and validate schema plus actual parser compatibility offline.
3. Add CLI dispatch before workspace discovery, deterministic discovery, actionable
   file/JSON-location errors, nonzero failures, and mode-specific reports.
4. Test single/collection, malformed JSON, fields/types, extensions/upstream mode,
   parser incompatibility, unreadable/empty inputs, exit status, and editing regression.
5. Document contract and publication workflow; run fmt, check, all-target Clippy,
   complete runnable test suite, build, and focused diff/security review. Commit locally.

## Decisions and evidence

Initial inspection: FeatureMount accepted objects only and ignored extra fields;
project configuration accepted strings only. Share a parser at config/mount.rs.
A JSON Schema dependency is justified to implement full upstream semantics rather
than a partial handwritten validator; disable network/file resolution features.

The upstream draft-07 schema is pinned, unmodified, at commit
1b2baddb5f1071ca0e8bcb7eb56dbc9d3e4a674f. Provenance/license/hash and explicit dcc
replacements are in `schemas/`. jsonschema 0.58.1 is MIT, actively maintained,
requires Rust 1.85 (below the installed stable 1.98), and is used with all default
features disabled; inspected Cargo feature tree confirms no resolution features.
serde_path_to_error 0.1.20 (MIT OR Apache-2.0) supplies precise parser locations.
Existing dependencies offer neither full JSON Schema validation nor Serde paths.

Shared `config/mount.rs` now serves project config, Feature deserialization, and
image labels. Validation permits string mounts only alongside that runtime support.
Objects retain upstream fields and reject ignored extras. String options/templates
are preserved. The command performs static dependency normalization/state checks,
never Feature acquisition or build-context generation. Upstream-only checks only
the original schema; arbitrary customizations remain schema-compatible there.

Discovery is direct-child and deterministic. Broken metadata entries fail; files
must be regular files (avoids blocking on a FIFO/device). Other children without
metadata are ignored, and no-feature collections fail.

## Verification

Final format, locked check, all-target Clippy, 592 unit tests and 114 runnable
integration tests, and locked build passed. The existing 38 Docker-backed tests
remain ignored as designed. Global flag regression found and fixed during final
verification. The 12 validation CLI tests and all 15 existing editor tests pass.
Full evidence and review: `.meta/quality/0092-feature-validation.md`.
