# 0007: Registry-Scoped Custom Certificate Authorities

Status: Accepted

Date: 2026-08-25

Owners:

- T-0071 architect and security review

Supersedes:

- None

Superseded by:

- None

## Context

`dcc` downloads Dev Container Features directly from HTTPS OCI registries through a
single `reqwest` client backed by rustls and the built-in public roots. Private
registries commonly use an organization CA, but adding that CA to the existing client
would authorize it for every registry, redirect target, and bearer-token endpoint.
Disabling verification, accepting HTTP, or changing the host trust store would weaken
unrelated traffic and is outside T-0070.

The configuration must work in inherited profile files, support deterministic local
TLS tests, fail before network activity when invalid, and preserve the unconfigured
path. Certificates are public trust material, not credentials, but their paths can
still disclose host layout and must not appear unnecessarily in diagnostics.

## Decision

Add `customizations.dcc.registryCAs`, an object whose keys are exact network
authorities and whose values are PEM-bundle file paths:

```jsonc
"customizations": {
  "dcc": {
    "registryCAs": {
      "registry.example.test": ".devcontainer/certs/registry-ca.pem",
      "auth.example.test:5443": "/etc/company/auth-ca.pem"
    }
  }
}
```

An authority is an ASCII DNS name or IP literal with an optional numeric port. It must
not contain a scheme, user information, path, query, fragment, whitespace, or a trailing
dot. DNS names are lowercased. IPv6 literals must use brackets. An omitted port means
443; explicit `:443` is canonicalized to the same key. Duplicate canonical authorities
are an error, including spelling variants in one object. Feature references use the
same canonicalization before lookup.

There is no CLI flag or environment-variable alternative in this revision. This keeps
trust configuration reviewable in the selected devcontainer configuration. Across
`customizations.dcc.extends`, the maps are merged by canonical authority and the child
value replaces the parent value. Unrelated authorities are retained. This map merge is
the complete precedence rule.

Relative paths are resolved against the directory containing the configuration file
that declares that map entry. Resolution happens before `extends` merging so an
inherited entry retains its origin; an overriding child entry uses the child's
directory. Absolute paths are accepted for centrally provisioned CAs. Tilde,
environment, devcontainer, and dcc variable expansion is not performed. Each path must
exist, be a regular readable file no larger than 1 MiB, and contain one or more PEM
`CERTIFICATE` blocks that `reqwest`/rustls accepts as roots. A bundle may contain
multiple certificates. Empty
files, malformed blocks, trailing non-PEM data, unsupported objects such as private
keys, and a bundle with no certificate are errors. Every configured entry is validated
when the selected configuration is loaded, even if no Feature uses that authority.
Byte-identical certificates within a bundle are harmless and may be deduplicated;
conflicting map entries are never silently combined.

The OCI transport keeps a public-root-only client and lazily builds a client for each
configured authority. An authority client retains the built-in public roots and adds
only that authority's configured bundle. Before every request, dcc canonicalizes the
target HTTPS URL and selects the exact matching client; a custom root is never added to
a client used for another authority. The absence of `registryCAs` takes the existing
public-root path and does not read CA files.

Automatic redirect following is disabled in the underlying clients. OCI requests may
follow at most ten redirects under these rules:

- every target must be HTTPS in production;
- each target is independently matched to its exact configured authority, otherwise it
  uses public roots only;
- `Authorization` is retained only for a same-origin redirect and is removed before a
  cross-origin request;
- redirect loops, missing or invalid `Location`, HTTPS-to-HTTP downgrade, and hop-limit
  exhaustion are contextual errors.

A bearer challenge realm must remain an absolute HTTPS URL in production. The realm
receives custom trust only when its own exact authority is present in `registryCAs`;
the registry's CA is not delegated to a different realm authority. Existing query
parameters are preserved and dcc appends the challenge service plus its own requested
repository scope. Tokens and response bodies remain absent from logs and errors. Token
cache keys remain registry-and-scope bound; tokens are attached only to registry
manifest/blob requests and never to the realm request or a cross-origin redirect.

Diagnostics name the operation and logical authority (for example, invalid CA bundle
for `registry.example.test`, TLS contact failure, rejected redirect, or token realm
failure). They may name the configured path when a local read fails, but never include
PEM contents, bearer tokens, response bodies, or URL user information/query values.
Certificate-chain and hostname-verification failures remain distinct through their
error chains; there is no retry with disabled verification or HTTP.

## Options Considered

| Option | Pros | Cons | Notes |
| --- | --- | --- | --- |
| Add roots globally to the existing client | Small implementation | A private CA becomes valid for every destination | Rejected: violates narrow trust binding. |
| Environment variable or CLI flag | Convenient for ephemeral invocation | Hidden precedence and less reviewable trust changes | Rejected for this revision; config plus absolute paths covers managed hosts. |
| Registry entry with an array of files | Directly represents several sources | More merge and duplicate semantics | Rejected; a standard PEM bundle already represents multiple roots. |
| Apply the registry CA to its advertised token realm | Supports split-host private auth with one entry | Registry-controlled challenge broadens the CA's authority | Rejected; configure the realm authority explicitly. |
| Require same-origin redirects | Very narrow | Breaks normal OCI blob-storage redirects | Rejected in favor of per-target trust selection and credential stripping. |
| Replace public roots for configured authorities | Strong isolation from public roots | Unexpectedly rejects publicly chained replacement certificates | Rejected; the feature adds roots and does not replace defaults. |
| Disable verification or permit HTTP | Easy local setup | Enables interception and credential/artifact compromise | Rejected and out of scope. |

## Consequences

Positive:

- Private trust is explicit, exact-authority-bound, and additive to public trust.
- Redirect and token-realm behavior cannot silently spread a private CA or bearer token.
- Configuration inheritance has one deterministic precedence rule.
- Production adds only a direct `rustls-pemfile` dependency to strictly enumerate PEM
  objects and feed certificate DER into the existing reqwest/rustls transport.

Negative:

- Split registry/auth/blob hosts using one private PKI require one map entry per
  authority.
- Config loading must preserve CA-path provenance through `extends` resolution.
- Manual redirect handling adds code and tests to the OCI transport.

Neutral or follow-up:

- T-0072 implements parsing, validation, client selection, redirects, tests, and user
  documentation.
- T-0073 may add test-only `rcgen`, `rustls`, and `tokio-rustls` direct dependencies for
  an in-process TLS fixture; these are not production trust dependencies.

## Confidence

Confidence: High

Why: exact-authority selection closes the global-root failure mode while supporting
standard private-registry, split-auth, and OCI redirect topologies explicitly.

## Review Trigger

Revisit this decision when:

- credentials beyond bearer-token challenges are added;
- OCI interoperability requires redirects that cannot obey per-target trust and
  credential rules;
- users demonstrate a need for non-file trust sources or invocation-only overrides; or
- reqwest/rustls changes root-store or redirect semantics materially.

## Sources

- `.meta/tasks/0070-custom-ca-registry-initiative.md`
- `src/features/oci.rs`
- `Cargo.toml` and `Cargo.lock` (`reqwest` 0.12 with `rustls-tls`; rustls and
  tokio-rustls are already locked transitively)
