# T-0063 Release CI Reuse Threat Model

## Scope

- Change: Reuse autotag's successful CI result instead of rerunning CI inside the
  called release workflow.
- Assets or data: Release-gate integrity, tag-to-commit identity, release artifacts,
  and repository tags.
- Users, systems, or agents involved: Version-bump, autotag, reusable CI, release,
  and GitHub-hosted runners.
- Trust boundaries: Commit identity passed between reusable workflows; a trusted
  caller's assertion that CI already passed; direct tag pushes entering release.

## What Can Go Wrong

| Threat | Impact | Likelihood | Existing Control | Gap |
| --- | --- | --- | --- | --- |
| Autotag reuses CI from a different commit. | An unverified commit can be tagged and released. | Medium | Reusable CI accepts an explicit ref. | The bump workflow previously did not expose its new commit SHA. |
| A failed or cancelled direct-tag CI is treated like an intentional skip. | Release builds proceed after a failed gate. | Low | Build already depends on the CI job. | A conditional CI job requires an explicit result-aware build condition. |
| Intentionally skipped reusable CI poisons the implicit status check of the final publication job. | The tag and artifacts exist, but no GitHub Release is published while the workflow reports success. | High | The build matrix has an explicit skipped-ancestor override. | The downstream publication job also needs an explicit cancellation- and build-result-aware condition. |
| An untrusted caller claims CI already passed. | Release CI is bypassed. | Low | Only repository-owned workflows can call the local reusable workflow, and release permissions are unchanged. | The trust assertion must remain narrowly owned by autotag. |
| Branch movement changes the commit between CI and tagging. | CI and released source diverge. | Medium | GitHub exposes immutable commit SHAs. | Autotag previously checked out moving `main` after CI. |

## Mitigations

| Mitigation | Owner | Verification | Status |
| --- | --- | --- | --- |
| Export the bump commit SHA and pass it into autotag. | T-0063 | Workflow data-flow review and `actionlint`. | Done |
| Use the same explicit SHA for autotag CI and tag creation. | T-0063 | Static expression contract check. | Done |
| Permit CI reuse only through the positive `ci_already_passed` boolean supplied by autotag. | T-0063 | Caller/input review and `actionlint`. | Done |
| Run release CI by default and allow builds only after trusted reuse or `needs.ci.result == 'success'`. | T-0063 | Direct-tag/reusable-call control-flow matrix review. | Done |
| Require a successful aggregated build result at the final publication job with an explicit status function that overrides skipped-ancestor propagation. | T-0067 | Static release-workflow contract check and `actionlint`. | Done |

## Agentic Risks

- Untrusted instructions or prompt injection: Not applicable; no external text controls
  the reuse decision.
- Tool permission risk: No workflow permission was added or widened.
- Dependency, script, or generated-code risk: No dependency or generated code changed.
- Secret or sensitive-data exposure risk: Commit SHAs and boolean gate state are not
  secrets; no new logging exposes credentials.
- CI/CD or deployment permission risk: Release publication retains its existing write
  permission, while gate reuse is limited to repository-owned autotag control flow.

## Residual Risk

- Accepted risk: Repository maintainers can alter trusted workflow definitions; branch
  protection and workflow review remain the control for that authority.
- Approval or decision record: User instruction for T-0063; the streamlined path
  preserves the existing green-CI-before-tag guarantee.
- Review trigger: Any additional release caller, change to `ci_already_passed`, removal
  of exact-SHA plumbing, or change to release/tag permissions.
