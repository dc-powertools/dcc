# 0006: Require Defaults For Missing `containerEnv`

Status: Accepted

Date: 2026-08-24

Owners:

- Product owner
- T-0056 implementer

Supersedes:

- [Decision 0005: Match Dev Container `containerEnv` Empty And Default Semantics](0005-container-env-substitution.md)

Superseded by:

- None

## Context

Decision 0005 matched the Dev Container reference implementation by turning an absent
`${containerEnv:VAR}` into an empty string. The product owner clarified that dcc's
intended design is stricter: silent substitution can hide a misspelling or unexpectedly
change a command, mount, environment value, work directory, or state path. Optional
variables must state their fallback explicitly.

Absence and an explicitly present empty value remain distinct. This decision changes
only missing keys; it does not make an empty value use a default, and it does not change
`${localEnv:…}` compatibility behavior.

## Decision

Apply this matrix to every runtime `${containerEnv:…}` consumer:

| Image environment | Token | Result |
| --- | --- | --- |
| `VAR` absent | `${containerEnv:VAR}` | Error naming `VAR` and the configuration consumer |
| `VAR` absent | `${containerEnv:VAR:fallback}` | `fallback` |
| `VAR` absent | `${containerEnv:VAR:}` | Empty string |
| `VAR` present as `""` | `${containerEnv:VAR}` | Empty string |
| `VAR` present as `""` | `${containerEnv:VAR:fallback}` | Empty string; the default is ignored |
| `VAR=value` | Either form | `value`; the default is ignored |

The rule applies to state paths, `workspaceFolder`, `runArgs`, mounts, `remoteEnv`,
container command arguments, and project or Feature lifecycle hooks. Each consumer adds
context while propagating the resolver error. Structured validation still runs after a
successful substitution, including when an explicit empty default or present-empty
value produces an empty string.

## Options Considered

| Option | Pros | Cons | Notes |
| --- | --- | --- | --- |
| Require an explicit default for absent values | Finds mistakes early; makes optionality visible; preserves intentional empty values | Deliberately differs from the upstream absent-to-empty behavior | Chosen product contract |
| Preserve decision 0005 | Matches the reference implementation | Missing variables silently alter configuration | Rejected by product clarification |
| Treat absent and present-empty identically | Simple strictness rule | Prevents a profile or base image from intentionally overriding a fallback with empty | Rejected |

## Consequences

Positive:

- Misspelled or unexpectedly absent image variables fail before profile container
  creation with actionable context.
- Defaults make optional-variable behavior explicit in configuration.
- Present-empty and present-nonempty image values retain their authored meaning.

Negative:

- Profiles relying on implicit absent-to-empty substitution must add `:` or another
  explicit fallback.
- A failed best-effort `HOME`/`USER` probe now causes unguarded references to fail
  instead of becoming empty.

Neutral or follow-up:

- `${localEnv:…}` retains the Dev Container compatibility matrix from its own resolver.
- Consumer-specific validation remains authoritative after successful substitution.

## Confidence

Confidence: High

Why: The product owner supplied the intended contract, and the fallible substitution
boundary reaches every runtime consumer. Unit and fake-Docker tests cover absence,
defaults, present-empty, present-nonempty, contextual propagation, and post-substitution
state validation.

## Review Trigger

Revisit this decision when:

- The product adds an explicit strict/compatibility mode for variable substitution.
- A new runtime-applied field consumes `${containerEnv:…}`.
- Image environment inspection or configured-user probing changes the definition of an
  absent key.

## Sources

- Product owner correction for T-0056, 2026-08-24.
- T-0056 brief: `.meta/tasks/0056-missing-container-env-error-brief.md`.
- Superseded upstream compatibility rationale: decision 0005.
