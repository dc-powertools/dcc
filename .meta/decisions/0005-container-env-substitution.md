# 0005: Match Dev Container `containerEnv` Empty And Default Semantics

Status: Superseded by [decision 0006](0006-require-missing-container-env-default.md)

Date: 2026-08-24

Owners:

- T-0051 implementer

Supersedes:

- The strict undefined/empty behavior previously encoded only in
  `vars::resolve_container_env` and project architecture

Superseded by:

- [Decision 0006: Require Defaults For Missing `containerEnv`](0006-require-missing-container-env-default.md)

## Context

Public documentation said an undefined `${containerEnv:VAR}` becomes an empty string,
while implementation and tests returned an error for both an absent variable and an
explicitly empty value. Either policy is plausible: compatibility favors empty
substitution, while strict failure makes typos easier to discover and prevents a missing
value from silently changing a command or path.

The upstream Dev Container specification says an unset variable is blank unless a
default is provided. Its reference CLI implementation is more precise: it returns a
present string unchanged (including `""`), uses the default only when the key is absent,
and otherwise returns `""`.

- Specification: <https://github.com/devcontainers/spec/blob/main/docs/specs/devcontainerjson-reference.md#variables-in-devcontainerjson>
- Reference implementation: <https://github.com/devcontainers/cli/blob/main/src/spec-common/variableSubstitution.ts>

## Decision

Match the upstream substitution contract:

| Image environment | Token | Result |
| --- | --- | --- |
| `VAR` absent | `${containerEnv:VAR}` | Empty string |
| `VAR` absent | `${containerEnv:VAR:fallback}` | `fallback` |
| `VAR` present as `""` | `${containerEnv:VAR}` | Empty string |
| `VAR` present as `""` | `${containerEnv:VAR:fallback}` | Empty string; the default is ignored |
| `VAR=value` | Either form | `value`; the default is ignored |

An explicit empty default (`${containerEnv:VAR:}`) therefore allows an absent variable
to resolve empty, but is not required for that result. Dcc continues to preserve colons
inside default text after the first separator.

Substitution does not decide whether the resulting field is valid. Each consumer keeps
its own boundary checks after substitution. In particular, state paths are normalized
and checked for absolute form, reserved paths, overlaps, and path kind after
`${containerEnv:…}` resolution. Lifecycle and command strings retain their surrounding
text and are passed through their existing command-form handling.

## Consequences

- Profiles relying on standard empty substitution no longer fail before runtime.
- A misspelled variable in a free-form command can become empty, matching upstream; a
  profile author who needs a non-empty value must enforce that in the command or choose
  a default.
- An empty or absent value cannot bypass state-path protection because validation runs
  on the resolved path. Tests cover both an empty path and an empty prefix that becomes
  the reserved `/cache` path.
- Explicitly empty and absent values are distinguishable when a default is present.

## Confidence

Confidence: High

Why: The normative reference describes unset-variable behavior and the maintained
reference CLI source directly defines absent, defaulted, and present-empty branches.
The local implementation and counterfactual consumer tests cover the same matrix.

## Review Trigger

Revisit this decision when:

- The Dev Container specification or reference CLI changes its environment lookup
  behavior.
- Dcc adds a strict compatibility mode that intentionally validates non-empty variables.
- A new structured consumer uses substituted text without validating its resulting form.

## Sources

- Dev Container specification variable reference:
  <https://github.com/devcontainers/spec/blob/main/docs/specs/devcontainerjson-reference.md#variables-in-devcontainerjson>
- Dev Container CLI `lookupValue` reference implementation:
  <https://github.com/devcontainers/cli/blob/main/src/spec-common/variableSubstitution.ts>

## Alternatives Considered

- **Keep hard errors for absent and empty values.** This catches typos early, but is an
  intentional compatibility deviation and conflates an explicitly empty variable with
  an absent key. Rejected.
- **Use defaults for both absent and empty values.** This preserves the previous dcc
  branch for explicit empty values, but differs from the reference implementation and
  prevents users from intentionally overriding a fallback with an empty value.
  Rejected.
