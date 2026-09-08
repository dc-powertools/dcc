# T-0086: Default Feature Repository Design

## Identity And Source

- Task ID: T-0086
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Product-owner design request
- Source reference and date: Configurable default Feature repository request, 2026-09-08
- Parent or split task IDs: None

## Goal

Allow a profile to name one default OCI Feature repository prefix so short Feature
references are convenient without changing the meaning of explicit OCI or local
references. For example, with `ghcr.io/dc-powertools/features` configured, `sudo`
expands to `ghcr.io/dc-powertools/features/sudo` and is fetched using the implicit
`latest` tag.

This task designs the behavior only. It does not implement it.

## Footprint Assessment

Verdict: **contained; no product-owner footprint approval is required before a future
implementation task.**

The capability fits the existing binary and configuration model. It requires one
scalar in `customizations.dcc`, shared reference normalization in the existing config
and Feature paths, focused changes to `dcc feature`, tests, and documentation. It does
not require a new command, flag, environment variable, global config file, persistent
store, background process, network protocol, dependency, permission, or registry
request. The OCI client continues to make the same requests after receiving a fully
qualified reference.

Adding a repository search list, aliases, machine-global defaults, or Feature
lockfiles would materially enlarge the product and precedence model. Those are not
required for this outcome and remain out of scope.

## Current Boundaries

- Project Features are ordered keys in the top-level `features` object.
- `customizations.dcc` already owns dcc-only, profile-reviewable settings and follows
  local `extends` inheritance.
- Local Feature references begin with `./` or `../`; every other reference currently
  reaches the OCI parser.
- The OCI parser currently requires an explicit tag even though the Dev Container
  reference implementation treats an omitted tag as `latest`.
- Feature resolution recursively adds `dependsOn` references, deduplicates by reference
  string, topologically sorts the result, and uses those strings in diagnostics and
  image metadata.
- `dcc feature` edits only the selected profile file and currently compares keys
  literally.
- dcc does not currently read or write a Dev Container Feature lockfile.

The upstream Feature repository documentation states that `:latest` is implicit when a
tag is omitted, and the reference CLI's `getRef` parser implements that behavior:

- <https://github.com/devcontainers/features#usage>
- <https://github.com/devcontainers/cli/blob/main/src/spec-configuration/containerCollectionsOCI.ts#L1700-L1872>

The Feature specification says `dependsOn` entries mirror the top-level Feature
semantics, but the short-name rule in this design is a dcc project configuration
extension rather than part of the portable Feature identifier format:

- <https://github.com/devcontainers/spec/blob/main/docs/specs/devcontainer-features.md#the-dependson-property>

## Configuration Contract

Add one optional project-owned field:

```jsonc
{
  "image": "debian:bookworm-slim",
  "features": {
    "sudo": {},
    "git:1": {},
    "ghcr.io/devcontainers/features/node:1": {},
    "./local-feature": {}
  },
  "customizations": {
    "dcc": {
      "defaultFeatureRepository": "ghcr.io/dc-powertools/features"
    }
  }
}
```

`defaultFeatureRepository` is an OCI repository **prefix**, not a URL and not a
fallback search service. It has no built-in value: when it is absent, all existing
explicit OCI and local references keep their behavior, while a short reference fails
with an error that points to this field.

The setting is allowed only in project configuration. A downloaded Feature declaring
`customizations.dcc.defaultFeatureRepository` is rejected just as one declaring
`registryCAs` is rejected; downloaded code cannot redirect the source of other code.
No CLI flag or environment variable overrides it, so identical checked-in profiles
resolve identically on different machines.

### Repository-prefix validation

Validate an effective value eagerly during config resolution, before network or Docker
access. It must:

- contain an OCI registry authority followed by at least one lowercase repository path
  component, such as `ghcr.io/dc-powertools/features`;
- allow a valid authority port, including the existing bracketed-IPv6 authority form;
- use OCI path components made from lowercase ASCII letters, digits, and valid
  internal `.`, `_`, or `-` separators;
- contain no scheme, credentials, query, fragment, whitespace, variables, tag, digest,
  leading slash, trailing slash, empty component, `.` component, or `..` component.

Represent the validated value with a small newtype rather than passing an unchecked
`String`. Reuse the existing `RegistryAuthority` parser for the authority. Validation
must not contact the registry.

## Reference Contract

Only a single-component Feature reference is shorthand. The supported forms are:

| Input | Meaning |
| --- | --- |
| `sudo` | Short name; append it to the configured prefix and use implicit `latest`. |
| `sudo:1` | Short name with an explicit tag; append the name and retain `:1`. |
| `ghcr.io/org/features/sudo` | Explicit OCI reference; never prefix it; use implicit `latest`. |
| `ghcr.io/org/features/sudo:1` | Explicit OCI reference and tag; never prefix it. |
| `./sudo`, `../features/sudo` | Existing local Feature reference; never prefix it. |

A short name follows one OCI path-component grammar and may have one valid OCI tag.
It cannot contain `/` or `@`. Empty names/tags and values with whitespace or path
syntax fail before I/O. Digest support, archive URLs, repository searching, and
multi-component relative shorthand are not added by this task.

For the requested example, source expansion is:

```text
sudo -> ghcr.io/dc-powertools/features/sudo
```

The fetch identity is normalized internally to
`ghcr.io/dc-powertools/features/sudo:latest`. Explicit untagged OCI references receive
the same implicit tag. Normalizing identity prevents `sudo`, the expanded untagged
form, and the expanded `:latest` form from being downloaded or installed as distinct
Features.

Errors name the source spelling, its declaring config file, and the relevant setting;
debug output may show the resolved qualified identity. Normal output does not add a
warning for valid shorthand.

## Declaration-Scoped Inheritance

Resolve Feature keys at their declaration boundary, before merging each file into its
child. This is required to preserve provenance.

For each file in an `extends` chain:

1. Resolve its parent first.
2. Determine the child's effective default: the child's own validated value when set,
   otherwise the inherited value.
3. Normalize only the Feature keys declared by that child using that effective default.
4. Reject two keys in the same file that normalize to the same identity.
5. Merge the already-normalized child map over the already-normalized parent map using
   the existing child-wins and declaration-order rules.

Consequently, a child that changes `defaultFeatureRepository` affects the shorthand it
declares and its descendants, but does not redirect Features declared by its parent:

```text
parent default A + parent feature sudo -> A/sudo:latest
child  default B + child  feature git  -> B/git:latest
```

If a child declares a shorthand that normalizes to the same identity as an inherited
Feature, it is the existing semantic key conflict: the child's options win while the
parent key keeps its ordering position. Equivalent spellings within one file are an
error rather than an order-dependent overwrite.

The default field itself follows scalar inheritance: child value wins and omission
inherits. An explicit JSON `null` has the existing optional-field meaning and does not
clear an inherited value. A profile that does not want to use an inherited default can
continue to use explicit references; no network fallback occurs merely because the
field exists.

## Resolution Pipeline

Centralize reference parsing and identity normalization in the Feature area so config
loading, dependency resolution, the OCI client, and `dcc feature` do not develop
different grammars.

```text
parse config file
  -> validate/elect effective default for this declaration
  -> qualify this file's short Feature keys
  -> assign implicit :latest to OCI identities
  -> reject same-file identity collisions
  -> merge with normalized parent Features
  -> resolve/download Features
```

The OCI client receives a parsed, qualified identity rather than reparsing arbitrary
shorthand. Authentication scope, custom-CA selection, redirect policy, digest checking,
and archive extraction remain unchanged. `customizations.dcc.registryCAs` naturally
matches the authority in the expanded reference.

Use normalized identities for queue deduplication, dependency-edge matching, context
IDs, diagnostics, and stored `devcontainer.metadata` Feature IDs. Preserve the Feature
metadata `id` behavior used for command short IDs and `installsAfter`.

### Feature-declared dependencies

Do not apply the project default repository to `dependsOn` keys read from downloaded
or local Feature metadata. Those keys must remain explicit OCI or local references;
an omitted tag may still mean `latest`. This avoids making a published Feature's
dependency source vary with the consuming project's unrelated default and prevents
downloaded metadata from steering a short dependency into a project-selected trust
domain.

Because both top-level and dependency references use the same qualified OCI identity,
an explicit dependency still deduplicates correctly against a top-level shorthand that
resolved to it.

## `dcc feature` Contract

Keep `dcc feature` within its current command and file-editing footprint:

- `dcc feature --add sudo` accepts the shorthand only when the selected file has an
  effective default through its own setting or `extends`, and writes the requested
  shorthand spelling with empty options.
- Add/remove comparison uses normalized identity for keys in the directly edited file,
  so `sudo`, its expanded untagged form, and its expanded `:latest` form do not become
  duplicate direct entries.
- `--remove sudo` removes an equivalent directly declared key. It never edits the
  parent file; an inherited Feature remains inherited under the existing contract.
- An exact raw-key removal remains available even when the default is missing or
  invalid, so the command can repair a broken shorthand entry.
- Summaries retain the spelling actually added or removed. `--dry-run` performs the
  same config/default/reference validation without writing.

This deliberately leaves shorthand in the profile, making the convenience visible and
reviewable. Such a profile is dcc-specific at that key; users who need another Dev
Container implementation to consume the same file should keep fully qualified Feature
keys.

## Alternatives Considered

| Option | Decision | Reason |
| --- | --- | --- |
| `customizations.dcc.defaultFeatureRepository` | Accept | Explicit, project-local, inheritable, and consistent with existing dcc configuration. |
| Machine-global config or environment variable | Reject | Makes checked-in profiles machine-dependent and introduces a new config source and precedence model. |
| CLI-only default flag | Reject | Does not let a checked-in short Feature key resolve consistently across build invocations. |
| Hard-code `ghcr.io/dc-powertools/features` | Reject | Does not meet configurability and couples dcc to one publisher. |
| Ordered repository search list | Reject | Adds extra network requests, ambiguity, latency, and dependency-confusion risk. |
| Arbitrary alias map | Reject for this outcome | Larger syntax and merge surface than the requested one-default, short-name behavior. |
| Expand after the full `extends` merge | Reject | A child default could silently redirect shorthand inherited from a parent. |
| Expand Feature metadata `dependsOn` shorthand | Reject | Makes publisher dependency identity depend on a consumer setting and broadens the trust boundary. |
| Add lockfile support with this feature | Reject for this outcome | Valuable separately, but it is a substantial reproducibility subsystem rather than a requirement for deterministic name expansion. |

## Implementation Slices

1. **Reference and configuration model**
   - Add the optional field and validated repository-prefix newtype.
   - Centralize short, explicit, local, tag, and implicit-`latest` parsing.
   - Normalize declaration-local Feature maps before `extends` merge and preserve
     existing ordering/override behavior.
2. **Resolver and editor integration**
   - Pass qualified parsed identities through the Feature queue and OCI boundary.
   - Keep `dependsOn` explicit while sharing tag/identity parsing.
   - Make `dcc feature` compare equivalent direct keys and validate shorthand without
     changing its command shape or parent-edit boundary.
   - Reject the setting in downloaded Feature metadata.
3. **Contract coverage and documentation**
   - Add focused config, inheritance, editor, resolver, and fake-OCI tests.
   - Update README, `docs/index.md`, `docs/features.md`, and architecture documentation.

These are implementation slices, not separate product approvals. They should ship in
one implementation task because partial support would let different entry paths assign
different identities to the same Feature.

## Automated Test Design

### Config and reference unit tests

- Valid prefix including an authority port; invalid scheme, credentials, uppercase or
  malformed path, tag/digest, trailing slash, whitespace, and variable cases.
- `sudo`, `sudo:1`, explicit tagged/untagged OCI, and local references normalize as
  specified.
- Short input without a default and malformed short names fail before network access.
- Untagged explicit and expanded Feature identities use `latest`; explicit empty tags
  still fail.
- Equivalent spellings in one Feature map are rejected with both source keys named.
- Arbitrary input cannot panic or bypass local/short/OCI classification.

### Inheritance and ordering tests

- A child inherits the parent default for its own shorthand when it omits the field.
- A child override applies to child shorthand without rebinding parent shorthand.
- A child equivalent identity overrides parent options and retains the existing parent
  ordering position.
- Independent Features retain declaration order after normalization.
- Invalid effective defaults and collisions identify the declaring file.

### Resolver, dependency, and OCI tests

- The OCI fixture receives the expected repository path and `latest` or explicit tag
  for top-level shorthand.
- A top-level shorthand deduplicates against an equivalent explicit `dependsOn`.
- A short `dependsOn` is rejected instead of using the project default.
- Existing custom-CA authority selection still applies to an expanded private-registry
  reference.
- Downloaded Feature metadata cannot set the default repository.

### `dcc feature` and CLI tests

- Add shorthand with a local or inherited default; reject add without one.
- Equivalent direct spellings report already present instead of creating duplicates.
- Remove shorthand matches a directly declared equivalent spelling but does not edit a
  parent declaration.
- Exact removal can repair a short key whose default is absent or invalid.
- Dry-run validates and reports without modifying the file; JSON summaries remain
  parseable.
- Existing fully qualified add/remove behavior remains unchanged.

The primary counterfactuals are a shorthand config and an untagged explicit OCI
reference: both fail before this implementation. The child-default inheritance test
must also fail any implementation that expands only after the full merge.

## Acceptance Criteria

- [ ] `customizations.dcc.defaultFeatureRepository` accepts one validated OCI prefix,
      inherits through `extends`, and has no machine-global override.
- [ ] With the example value, `sudo` expands to
      `ghcr.io/dc-powertools/features/sudo` and fetches its implicit `latest` tag;
      `sudo:1` retains tag `1`.
- [ ] Explicit OCI and local references are never prefixed, and untagged explicit OCI
      references also use the upstream-compatible implicit `latest` tag.
- [ ] Parent shorthand retains the parent declaration's effective repository when a
      child changes its default.
- [ ] Normalized identity preserves Feature order, child overrides, queue deduplication,
      dependency matching, diagnostics, and image metadata without duplicate installs.
- [ ] Project defaults do not qualify Feature-declared `dependsOn`, and downloaded
      Feature metadata cannot configure a default repository.
- [ ] `dcc feature` adds, detects, removes, dry-runs, and reports shorthand consistently
      while editing only the selected profile.
- [ ] No new dependency, config source, persistent artifact, network fallback, command,
      or privilege is introduced.
- [ ] User and architecture documentation explains shorthand portability, implicit
      `latest`, inheritance provenance, validation, and explicit-reference escape hatches.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Child default rebinds inherited shorthand | A profile silently downloads root-executed code from another repository. | Normalize each file before merge and test provenance explicitly. |
| Alias spellings install twice | Duplicate scripts, conflicting options, or incorrect ordering. | Use one qualified identity everywhere and reject same-file collisions. |
| Downloaded metadata uses consumer default | Publisher dependency identity changes across projects. | Restrict shorthand to project Feature declarations and reject the setting in Feature metadata. |
| Mutable implicit `latest` changes build output | Builds are not reproducible. | Match upstream semantics, document explicit tags, and keep lockfile work separate. |
| Global override changes checked-in meaning | Builds differ by developer machine. | Provide no global config, env, or CLI override. |
| Shorthand reduces cross-tool portability | Another Dev Container tool cannot resolve the key. | Keep it in the explicit dcc namespace and document fully qualified keys as the portable form. |
| Prefix accepts URL-like or ambiguous input | Wrong authority/path or unsafe diagnostics. | Strict offline validation, existing sanitized authority handling, and no schemes/credentials/query data. |

## Verification Plan For Implementation

- Focused: config/merge/resolve tests, Feature reference and OCI fixture tests, Feature
  resolver tests, and `dcc feature` tests.
- Required full checks: `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo build`.
- Manual: example config resolution, parent/child provenance, exact error and debug
  output, docs/read-through, and `git diff --check`.
- Live Docker is not required for name expansion because the behavior ends at the
  existing OCI/download and Docker build boundaries; the Docker-free OCI fixture covers
  the changed network target deterministically.
