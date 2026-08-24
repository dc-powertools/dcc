# T-0048 Brief: Feature Acquisition Trust-Boundary Tests

## Identity And Source

- Task ID: T-0048
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: Parent T-0043, authorized by user follow-up
- Source reference and date: Test-quality audit, 2026-08-24
- Parent or split task IDs: T-0043

## Goal

Untrusted OCI Feature responses, metadata, and archives are accepted only when they
satisfy explicit authentication, integrity, schema, and extraction contracts, with
deterministic tests at each trust boundary.

## Background

Existing OCI tests cover reference parsing, layer selection, basic extraction, and a
no-panic authentication-header parser. They do not exercise bearer authentication,
token caching, HTTP status handling, or digest match/mismatch. Separately,
`parse_feature_meta` converts malformed JSON into default metadata, and its test
explicitly preserves that silent fallback. Missing `devcontainer-feature.json` is also
accepted without a clearly recorded compatibility decision.

## Scope

In scope:

- A local deterministic HTTP registry fixture covering bearer challenges, token
  requests, token reuse, manifest/blob statuses, and bodies.
- Digest success and mismatch tests at the downloaded blob boundary.
- Explicit outcomes for missing metadata, malformed JSON, and schema-invalid fields;
  malformed supplied metadata must not silently become an empty Feature.
- Archive traversal, absolute path, link, and special-entry safety tests as applicable
  to the extractor.
- Clear errors that avoid including bearer tokens, credentials, or sensitive response
  content.

Out of scope:

- Restoring removed lockfiles or promising registry availability.
- Broad OCI-client replacement unless the local design cannot be tested safely.

## Acceptance Criteria

- [ ] Tests cover authentication challenge parsing with reordered/optional parameters,
  token acquisition, token reuse, and auth failure.
- [ ] Tests cover manifest/blob non-success statuses and useful sanitized errors.
- [ ] Correct content passes digest verification and a one-byte mutation fails.
- [ ] Malformed supplied Feature metadata is a clear error; the chosen behavior for
  missing metadata is documented and tested.
- [ ] Unsafe archive paths or entries cannot escape the extraction root.
- [ ] No test is satisfied merely because parsing does not panic.
- [ ] Counterfactual evidence shows digest, malformed-metadata, and archive-safety tests
  catch the pre-change or deliberately weakened behavior.

## Workflow Route Rationale

- Cataloged route and risk: Initiative / High.
- Why this route: Authentication, HTTP, integrity, parsing, and extraction need a shared
  deterministic fixture but distinct assertions.
- Why this risk gate: Registry content is external input executed during image builds;
  digest verification and archive confinement are supply-chain boundaries.

## Verification Plan

- Automated checks: local-registry integration tests, focused parser/extractor tests,
  full tests, lint, format, and build.
- Security check: project security checklist for external input, secrets, filesystem
  paths, errors, and dependencies.
- Manual check: review errors and logs for token or credential disclosure.

## Done When

The suite rejects corrupted, malformed, unauthorized, and unsafe Feature inputs for the
right reasons while accepting a valid deterministic registry flow.
