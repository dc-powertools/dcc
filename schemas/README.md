# Bundled Feature validation contract

`devContainerFeature.schema.json` is an **unmodified** copy of the upstream
[Dev Container Feature schema at commit
1b2baddb5f1071ca0e8bcb7eb56dbc9d3e4a674f](https://github.com/devcontainers/spec/blob/1b2baddb5f1071ca0e8bcb7eb56dbc9d3e4a674f/schemas/devContainerFeature.schema.json),
the latest change to that file as of 2026-09-28 (commit dated 2024-01-22).
It uses JSON Schema draft-07. The upstream CC BY 4.0 license is included in `LICENSE`.

SHA-256: `671fcd80cbb3510793412746b35505d12b6a4b8e68d38a01960e440aa5af32eb`.

`dcc-feature-extensions.json` owns the explicit dcc contract as replacements at
JSON Pointers in the upstream schema. It permits string mounts, `remoteEnv`, and
legacy `scripts`, and validates `customizations.dcc.commands` and `.state`.
Directory exclusions require the fail-closed `type: "directory-with-exclusions"`
discriminator and a nonempty `exclude` array of `{ "path": "relative/child",
"target": "/absolute/image/location" }` objects. Older dcc parsers reject this new
type rather than silently persisting excluded software. The parser additionally
checks literal relative children, overlap and that targets lie outside every
persistent root. `directory` and legacy `file` entries cannot contain `exclude`.

Other tools' customizations remain unrestricted, as upstream specifies. Object
mounts retain the upstream fields; `readonly` is not an object property.

Both files are compiled into the binary. The `jsonschema` dependency has all
network and file reference resolution features disabled. User metadata cannot
select or supply a schema. Diagnostics expand failed `oneOf`/`anyOf` branches to
show the underlying field errors without changing upstream schema semantics.

To update the contract, fetch a reviewed immutable upstream commit, preserve its
bytes, update this revision/hash and license, then review extension pointer targets
and run the Feature validation, shared mount, and full project tests. Schema
acceptance alone must never add a runtime extension. See
[Feature validation usage](../docs/features.md#validating-features-before-publication).
