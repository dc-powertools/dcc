# T-0096: Design Quality And Implementation Readiness

Date: 2026-09-28. Route: Decide. Risk: High (runtime architecture and trust boundaries).
Review: primary agent's focused architecture/security/QA review, without delegated
agents. Scope is a design artifact, not an implemented feature or runtime validation.

## Acceptance And Readiness

| Criterion | Evidence | Status |
| --- | --- | --- |
| Match accepted placement, timeout, recovery and migration policy | Design maps to decisions 0008/0009; final socat -t 2 amendment supersedes the earlier absolute-deadline interpretation. | Pass |
| Reuse without host listener collision or applying changed config | Separate creation/reuse paths, immutable snapshot/assets, fixed image identity, read-only drift comparison and stop-before-build preflight. | Pass (design) |
| Concrete protocol and failure behavior | Port schema/allocation, service states, readiness ownership, bounded recovery, exact-attempt fallback, snapshot/control operations specified. | Pass (design) |
| Bounded implementation and security surface | Packaged utility and shell supervisor retained; no custom binary, sidecar, host daemon, deferred build machinery or new Rust dependency required. Threat card records remaining boundaries. | Pass (design) |
| Reviewable delivery and verification | Five implementation slices, fake-Docker and live-platform matrix, explicit migration and rollback; no runtime results claimed. | Pass (design) |

Readiness: **Ready with concerns** for a separately authorized implementation.
Concerns are qualification work, not unresolved product choices: distro utility and
shell behavior, process-group cleanup, procfs permissions, snapshot UID mapping,
Docker/Desktop publication behavior and exact failed-start states. Begin with the
relay/control contract tests and package qualification before wiring the host path.
If those reveal a need for a new binary, broad supervision framework, weaker policy,
or unsupported fallback assumptions, pause for input rather than expanding scope.

## Focused Second Pass

Lens: assume concurrent attach, failed IPv6 startup, or changed config caused an
incorrect success or host-side mutation; trace each through the design.

| Finding | Resolution |
| --- | --- |
| A bare listening-port probe can observe the wrong process. | Require inode/PID ownership and a bounded readiness deadline. |
| One bootstrap failure status could make degraded relay block shell access. | Separate wait-hooks from strict new-launch wait-ready and relay health. Existing hook failure remains distinct. |
| New mutable image tag could incorrectly authorize old running protocol. | Gate on actual image ID; compare current tag only as drift evidence. |
| Snapshot data from the container could become host instructions. | Limited DTO, bounded validation, no host execution/mount/cleanup fields, host-derived fingerprint input paths. |
| Failed docker run retry could duplicate hooks or delete a concurrent winner. | Create/start exact ID/token; retry only proven-never-started attempt, otherwise error. |
| Host-loopback bindings on old Docker do not establish the intended boundary. | New forwarding qualification requires Engine >=28; custom routed networks excluded; stop/reuse remain accessible. |
| Shell service failures could orphan per-connection workers. | Own process groups, cleanup before restart, stop retries when shutdown starts or cleanup fails. |
| Asset directory reused by build/start could alter frozen hooks. | Unique immutable instance payloads, separate build-prep assets and stop-before-build rule. |

## Verification Results

Required for this design task: source/policy consistency review, security review,
local link/path validation, and documentation diff/whitespace inspection. Product
builds, tests and container execution are not applicable to this documentation-only
task and were not run, honoring the user's no-reproduction/environment constraint.
Future runtime checks are explicitly required by the design before feature delivery.

Observed: source and upstream documentation inspected; socat original source read
without execution; timeout discrepancy escalated and resolved by explicit user choice.
Local Markdown link checks and git diff --check passed. Final architecture, security,
source/policy consistency, documentation scope, and secret-exposure reviews passed.
The project cursor was reduced to its five recent outcomes and 78-line hard-budget
compliant form; historical results remain in the task catalog.

## Scope And Residual Risk

Architecture spans several subsystems, so the implementation is split into slices.
The design is kept together because the snapshot, lifecycle and publication contracts
must be reviewed as one feature. No product code, public behavior, release metadata,
or package installation has changed. The documented platform assumptions remain
unverified until implementation can run in a container-capable environment.

Completion applies to the design only; it does not claim the port-collision bug is
fixed. Canonical product policies remain in decisions 0008/0009, concrete mechanics
in the task design, and security analysis in the matching threat card.
