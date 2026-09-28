# T-0097: Implement Container-Owned Forwarding

Authority: user "Implement this", 2026-09-28, r1. Implement the approved
[T-0096 design](0096-container-relay-design.md), including socat -t 2, frozen runtime
configuration, stop-before-build, and the 0.2.0 compatibility boundary. No push/release.
No container launch or reproduction is allowed in this environment.

Work sequence: relay/configuration contracts; snapshot and immutable assets; reuse
and creation/control integration; non-container regression tests; docs and review.
One primary agent owns the changes. No delegation requested.

Acceptance and required checks: follow the design matrix; run format, locked check,
all-target Clippy, all runnable tests and build without enabling ignored Docker
suites. Add fake-Docker and local shell contract tests. Container/platform validation
remains required before release and unavailable here; report it explicitly rather
than claiming the full feature has been runtime-qualified.

Readiness: ready with the design's bounded qualification concerns. Escalate unexpected
complexity or policy changes. Keep implementation slices together if the atomic
supervisor protocol change prevents independently passing intermediate commits.

Initial checkpoint: intake and source inspection completed before implementation.

Implementation checkpoint (2026-09-28): container-owned forwarding and frozen
reuse are implemented. Production modules are `runtime.rs`, `runtime_snapshot.rs`,
`forward.rs`, three relay shell assets, and the supervisor/build/config integration.
The CLI/image protocol is now 0.2.0. Public docs explain migration, port range,
network requirements, closing wait, degraded access and config deferral.

Local evidence and review: [quality record](../quality/0097-container-relay-implementation.md).
Full runnable checks and focused local socat/PID 1 tests are the handoff gate.
Required remaining work is the approved live Docker/platform matrix in a capable
environment. No containers were launched here. Keep status Needs verification until
that evidence exists; no new product-design ambiguity or permission request is open.

Final runnable verification: fmt, locked check, all-target Clippy with warnings
as errors, 702 default runnable tests, all five explicitly selected local relay/
supervisor tests, locked build, local Markdown links and diff whitespace passed.
Implementation is ready for the required live-platform qualification. Local commit
contains the atomic protocol change; nothing was pushed or released.
