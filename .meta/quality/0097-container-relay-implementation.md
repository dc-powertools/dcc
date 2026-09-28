# T-0097: Implementation Quality

Date: 2026-09-28. Route: Initiative. Risk: High.
Owner/reviewer: primary Codex agent; focused architecture/security/QA review without
sub-agents. Authority and scope: [task](../tasks/0097-container-relay-implementation.md),
[approved design](../tasks/0096-container-relay-design.md), and
[threat controls](../threat-models/0096-container-relay-design.md).

## Scope And Readiness

Implement Docker-owned loopback publications and supervisor-owned in-container
socat relays, frozen runtime configuration, safe creation fallback, and explicit
0.2.0 migration. No sidecar, host daemon, new Rust dependency, container launch,
issue reproduction, push, or release. Readiness inherited the design's bounded
platform qualification concerns. The protocol-dependent slices are kept in one
commit so each committed tree remains coherent and runnable.

## Acceptance Evidence

| Criterion | Evidence | Status |
| --- | --- | --- |
| Deterministic ports and fixed relay interface | Allocation/exhaustion/boundary unit tests; explicit loopback publication argv; numeric argument rejection and executable asset tests. | Pass locally |
| Container lifecycle ownership and socat -t 2 | Local Linux socat tests: listener ready before app exists, foreign listener rejected, half-close response, idle connection survives, closing wait, restart after listener death, lifetime three-retry budget, active-worker cleanup. Local generated PID 1 test proves relay does not prevent one-shot teardown. | Pass locally |
| Frozen config and safe host boundary | Bounded snapshot validation, unknown fields/identity/port rejection, secret-safe errors, fingerprint inherited/direct/optional inputs. Fake Docker proves frozen named commands, unchanged snapshot bytes, malformed/deleted config reuse, absent/empty environment distinction, explicit resource drift, private file modes. | Pass locally |
| Stop before build/refresh/reseed | Fake-Docker refusal checks with zero build calls. Existing dry-run suite remains Docker-free. | Pass locally |
| Safe creation/fallback | Fake Docker checks exact v4/v6 flags, one IPv4 retry only after never-started proof, no removal/retry of uncertain started attempts, Engine floor and wildcard binding rejection. Source review verifies exact ID/token ownership and competing-winner branch. | Pass locally; actual daemon behavior pending |
| Immutable assets and cleanup | Concurrent directory creation and symlink rejection; private instance ownership guard; fake Docker retains stopped-container references and uncertain references, prunes released/unreferenced directories, retains in-progress plans. | Pass locally |
| Compatibility/docs | 0.2.0 label boundary; old version gates and best-effort legacy stop regressions; public forwarding/range/drift/migration docs and architecture updated. | Pass locally |
| Live container/platform matrix | Docker Engine/Desktop publication, concurrency, IPv6 fallback, remote paths, non-root UID/procfs permissions, package behavior on Debian/Ubuntu, Alpine and Fedora-family images. | Not run: explicitly prohibited/unavailable |

## Focused Review

- A persistent process-group anchor and procfs identity checks prevent signaling a
  recycled group after listener death. Uncertain cleanup stops further restart.
- Readiness checks socket ownership instead of reaching an arbitrary listener.
  Initial relay failure preserves diagnosis; hook-only readiness permits commands
  after relay degradation. Infrastructure never registers as foreground work.
- The current tag cannot authorize an old running supervisor: compatibility uses
  the actual immutable image ID. Tag movement is only drift evidence.
- Snapshot data is a limited container-execution DTO. It cannot specify host
  hooks, mounts, config file reads or cleanup paths. Reads have a 16 MiB cap;
  payloads are private and never printed. Runtime debug/transport errors omit
  resolved command/environment text; ordinary program/hook output remains visible.
- Reuse occurs before current-config planning or mutable cache/state preparation.
  Named commands are resolved against frozen scripts without a second substitution.
- Asset guards survive creation, so another CLI cannot prune a not-yet-created
  launch. Only released directories with verified absent Docker references qualify.
  A host crash can leave an unmarked directory for manual cleanup.
- Reviewed create/start failure branches, exact-attempt deletion, binding checks,
  stop draining, one-shot promotion, reserved launch labels, snapshot permissions,
  dry-run separation, package provisioning and old-call-site removal.
- Removed obsolete host relay code and tests, and obsolete debug-rendering helpers
  and their tests. Preserved skip-lifecycle warnings.

## Verification Results

- `cargo fmt --check`: passed.
- `cargo check --locked`: passed for dcc 0.2.0.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --locked --quiet`: passed, 580 unit plus 122 runnable integration
  tests; 43 tests ignored by default, including the five local utility tests below.
- `cargo test --test relay_local -- --ignored --nocapture`: all four passed.
- `cargo test --bin dcc supervisor::tests::relay_does_not_keep_oneshot_supervisor_alive -- --ignored --exact --nocapture`: passed.
- `cargo build --locked`: passed.
- `git diff --check` and local Markdown link checks: passed.
- Shell syntax and fixed socat argv checks are covered by the unit suite; the
  installed socat capability check also passed.

No Docker-dependent ignored test was enabled. The host test utility was Debian
socat 1.8.0.3-1+deb13u1; this is not a distro-container qualification result.

## Consistency And Handoff

User docs, source contracts, architecture, context, task/cursor and migration agree.
No release workflow, external service, repository remote or state migration changed.
Rollback: stop the runtime and recreate with a matching older CLI/image pair.
The required live matrix remains outstanding; local tests do not establish Docker
Desktop behavior, daemon failure-state guarantees, or cross-distro qualification.
Task completion is **Needs verification**, not fully runtime-qualified delivery.
The ten-completed-task hygiene counter remains at nine until required qualification
completes; this multi-file change received its own focused consistency review.
