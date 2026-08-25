# Skipped Release Publication

- Date: 2026-08-24
- Status: Resolved locally pending the next authorized release run
- Related task: T-0067
- Affected run: GitHub Actions run 32752763536 for commit `c3648ed4ea04942f3e598604e7c54a83e01aed83`

## Impact And Detection

The automatic release path created tag `v0.1.3`, passed exact-commit CI, built and uploaded all four platform artifacts, but skipped `Create Release`. The workflow still concluded `success`, so the absent GitHub Release was the visible symptom.

## Contributing Factors

- T-0063 intentionally skipped reusable release CI after autotag had already verified the exact commit.
- The build matrix explicitly overrode skipped-job propagation, but the downstream publication job retained GitHub Actions' implicit `success()` condition.
- Static workflow review covered the CI-to-build gate but not the build-to-publication edge.
- The repository history contained an earlier correction for the same skipped-ancestor behavior, but that constraint was not retained in automated coverage.

## What Worked

- Exact-commit CI, tag creation, all release builds, packaging, and artifact upload succeeded.
- GitHub's jobs API made the skipped reusable CI and skipped publication conclusions explicit.

## Corrective Actions

- Add an explicit cancellation- and build-result-aware condition to `Create Release`.
- Add a static workflow contract check covering both the trusted-reuse and direct-tag paths through publication.
- Extend the release CI-reuse threat model with skipped-ancestor propagation at the final publication edge.

## Residual Verification

Local validation cannot execute GitHub's hosted job scheduler. The next owner-authorized automatic release must confirm that `Create Release` runs after all four builds succeed.
