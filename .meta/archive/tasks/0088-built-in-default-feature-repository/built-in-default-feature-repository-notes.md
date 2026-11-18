# T-0088: Built-In Default Feature Repository Notes

## Frame

- Goal: Resolve short project Feature names beneath
  `ghcr.io/dc-powertools/features` when no project override is declared.
- Scope: the shared default election, config inheritance tests, `dcc feature` behavior,
  and user/architecture documentation.
- Non-goals: changing explicit OCI/local references, applying shorthand to Feature
  metadata, adding a repository search, global config, dependencies, commands, or
  network fallback.
- Compatibility: `customizations.dcc.defaultFeatureRepository` remains a validated,
  declaration-scoped override; root omission or `null` selects the built-in value.
- Risk: a missing fallback at either config load or editor time could leave two
  different short-name meanings.
- Done when: absent-setting config and editor paths resolve `sudo` to
  `ghcr.io/dc-powertools/features/sudo:latest`, configured overrides still win, docs
  match, and all required Rust checks pass.

## Authority Amendment

The product owner's 2026-09-09 request supersedes T-0086's earlier statements that the
setting has no built-in value and that hard-coding this repository was rejected. The
rest of the T-0086/T-0087 validation, provenance, identity, dependency, editor, and
footprint contract remains authoritative.

## Result

- The shared project Feature normalizer now selects the validated configured/inherited
  override when present and otherwise uses `ghcr.io/dc-powertools/features`.
- Missing and root-level `null` overrides resolve `sudo` to
  `ghcr.io/dc-powertools/features/sudo:latest`; explicit short tags are retained.
- Parent declarations using the built-in repository remain bound to it when a child
  configures another repository. Built-in and fully qualified aliases participate in
  the existing collision and editor identity rules.
- `dcc feature` adds and matches short names without configuration. Invalid configured
  overrides retain the exact-removal repair path.
- Feature-authored `dependsOn` remains explicit-only. No dependency, command, global
  config source, persistence, privilege, repository search, or network fallback was
  added.
- The absent-setting test failed against the prior implementation with the expected
  `short Feature references require customizations.dcc.defaultFeatureRepository`
  error, then passed with the fallback.
- `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test` (591 unit and 93 runnable
  integration tests; 38 intentionally ignored), `cargo build`, and `git diff --check`
  passed. Focused correctness, safety, maintainability, test, documentation, and
  amended-contract review found no remaining issue.
