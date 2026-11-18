# T-0062 CI Ref And Fixture Threat Model

## Scope

- Change: Make executable test fixtures fork-safe and pass the requested release tag
  into every reusable CI checkout.
- Assets or data: CI result integrity, release artifact source identity, repository
  contents, and temporary test files.
- Users, systems, or agents involved: GitHub Actions, repository workflows,
  `actions/checkout`, Rust test processes, and release maintainers.
- Trust boundaries: Workflow inputs crossing into checkout selection; the
  multithreaded Rust test process crossing into child processes and temporary files.

## What Can Go Wrong

| Threat | Impact | Likelihood | Existing Control | Gap |
| --- | --- | --- | --- | --- |
| Release CI checks a different commit from the requested tag. | Green CI can authorize artifacts built from unverified source. | Medium | Build and release jobs already use the requested ref. | Reusable CI previously ignored it. |
| An untrusted ref reaches a shell command. | Command injection or unintended source checkout. | Low | The ref is passed as structured `actions/checkout` input, not interpolated into a shell; CI has read-only contents permission. | None introduced. |
| Direct push or pull-request CI loses its event commit semantics. | CI could test the wrong revision. | Low | Empty reusable input falls back to immutable `github.sha`. | None. |
| Temporary executable contents or paths are interpreted as shell code while being written. | Test-only command execution outside the intended fixture. | Low | Contents travel over stdin and the destination is positional parameter `$1`; the writer command contains no evaluated path or content. | None. |

## Mitigations

| Mitigation | Owner | Verification | Status |
| --- | --- | --- | --- |
| Declare an optional reusable-CI `ref` and use it in every checkout. | T-0062 | Static five-checkout contract check and `actionlint`. | Done |
| Forward the release workflow's selected tag to reusable CI. | T-0062 | Release workflow diff review and `actionlint`. | Done |
| Preserve direct-event behavior with `github.sha` fallback. | T-0062 | Expression review and `actionlint`. | Done |
| Write executable fixtures in a child without opening the target for writing in the multithreaded parent. | T-0062 | Process-churn regression, focused forwarding tests, and full suite. | Done |

## Agentic Risks

- Untrusted instructions or prompt injection: Not applicable; no external text controls
  workflow behavior.
- Tool permission risk: No permissions were added; reusable CI remains `contents: read`
  and existing release jobs retain their prior write boundary.
- Dependency, script, or generated-code risk: No dependency was added. The fixture
  helper uses fixed `/bin/sh`, `/bin/cat`, and positional arguments in tests only.
- Secret or sensitive-data exposure risk: No new secret input, output, or logging.
- CI/CD or deployment permission risk: Checkout identity changes, but release creation,
  token permissions, triggers, and publication behavior are unchanged.

## Residual Risk

- Accepted risk: A repository administrator can move a tag; this change verifies and
  builds the ref resolved by GitHub at job checkout time and does not add tag immutability.
- Approval or decision record: User instruction for T-0062; no separate decision needed
  for this reversible correction.
- Review trigger: Any change to workflow callers, ref derivation, checkout actions,
  release permissions, or tag immutability policy.
