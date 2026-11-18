# Migration verification and independent recovery

## Required verification and observed evidence

Baseline is Git revision `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce`. Review confirmed the captured catalog rows and all source artifacts equal that revision byte for byte. The [explicit mapping](migration-map.md) records every task and artifact, including source hashes.

| Check | Observed result |
| --- | --- |
| Catalog preservation | 126 canonical task READMEs; every original identity, authority/revision, status, dependency, route/risk, approval/blocker, next action and result preserved. T-0120 alone advances to execution r2 and completion. Necessary navigation correction changes the path in T-0024-D1's outcome. |
| Supporting preservation | 106 unique source-to-target mappings: 59 task details, 31 quality records, ten decisions, five threats, one incident. Every supporting file is linked and explained by its owner; all original catalog consumers retain shared links. Source bodies equal baseline after the documented navigation transformations. |
| Identity and dependency integrity | All 126 IDs unique across current/archive. Six legacy subtasks preserved. T-0004 and T-0043 child ranges expanded for resolution and acyclic-graph validation. Random-ID policy requires uniform six-letter allocation, collision regeneration in both namespaces and explicit integration collision repair; no shared allocator remains. |
| Index ownership and coverage | Five unrelated nonterminal tasks remain current after completion, 121 terminal tasks archived. Each task appears exactly once in its correct discovery index. Indexes contain no lifecycle or global scheduling fields. |
| Controls and current requirements | Primary pointer T-0108/r6 retained; Scheduling Running and pause None have one cursor owner. T-0097 no-launch boundary, T-0108 rollout/platform gates and T-0113 r8 selected/deferred work retained. All ten decisions and cross-cutting threats reconciled into current architecture/security without upgrading qualification claims. |
| Navigation | Local Markdown targets and affected anchors checked, including task indexes, cursor, core/templates and conventional docs. Full legacy paths and inline relative file references checked; source-comment decision reference corrected. Old paths exist only as clearly labeled mapping/provenance data. Final link count recorded in execution results. |
| Framework consistency | Startup/bootstrap recognizes Project State and Current Tasks. Root loop, ingestion, resumption, automation, quality, improvement, agent guidance and all 13 templates agree. Core remains Markdown-only; no optional harness adapters existed. |
| Scope and formatting | Whitespace check passes. The only source edit changes a comment's documentation path; source with that comment removed is identical. No manifests, product logic, tests or executable commands change. Application runtime tests are not applicable under the task's documentation-verification boundary. |

Verification method: parse original escaped-pipe catalog cells from the baseline Git tree; compare each corresponding README field; construct the explicit artifact map and compare transformed original bodies; resolve every canonical owner/consumer/index target; expand dependency ranges and walk the complete graph; scan inline Markdown links, anchors and literal/inline-relative paths; check retired path patterns and process/template ownership terms. Scripts were temporary execution aids outside the repo, not a framework runtime or required task database.

## Independent review

`integrity_review` independently compared the baseline snapshot with Git, checked all 126 identities/catalog fields and 106 artifact targets, reran integrity checks, and approved the atomic migration after findings were resolved. `framework` and `requirements` performed separate final consistency passes. One atomic commit is intentional: source paths, authoritative state, indexes and bootstrap detection must cut over together; splitting moves from ownership/reference updates would leave invalid intermediate states.

Resolved findings:

- Removed a newly revived empty-lockfile emission rule; T-0041 removed Feature lockfiles.
- Corrected a historical decision's inline relative reference and a stale old path in an archive-index summary.
- Rebasing preserved the target of former catalog-relative `README.md` links, rather than accidentally linking the new task README.
- Removed obsolete reseed CLI/preflight wording and repaired the source-map table.
- Removed duplicated stale observed toolchain metadata from context; the verified command catalog retains the sole observation owner.

## Fresh cold-start exercise

A separate agent started with no inherited conversation and read root instructions, framework README, cursor, current index and task READMEs before applicable current requirements. It recovered T-0108/r6 Needs verification, its next Desktop/macOS/updater qualification action and broad-rollout gate; and T-0113/r8 Ready, shared lifecycle flock, PID/start-time retirement, removal of separate ARRIVED, STOPPING-under-lock, container-owned cleanup and deferred result work. Historical supporting contents and archive entries were unnecessary in this phase. Standards was found as the sole verified-command owner. T-0120 r2 was correctly recognized as the explicitly selected migration while preserving the existing primary pointer.

A separate targeted lookup through current architecture evidence links recovered decision 0005's upstream absent-to-empty rationale, decision 0006's deliberate missing-variable error/default policy, and decision 0010's supersession of content-hash reconciliation by absence-only initialization. Historical records retain their original Accepted/status wording; current requirements and explicit supersession links determine applicability. Original historical measurements were not claimed as freshly reproduced.

## Budgets and exceptions

The former always-read catalog was 102,036 bytes. Final cursor and current discovery sizes are recorded in execution results; both satisfy hard/default line and byte limits. Root AGENTS is 17 lines/919 bytes. Current task READMEs and framework process docs meet their budgets. The historical index is deliberately separate and not startup context. Existing long design/analysis files remain optional supporting evidence unchanged except navigation; current checkpoints are bounded task READMEs, so preserving historical analysis does not require loading it to resume. The detailed established architecture guide remains a topic owner, read selectively rather than mandatory full startup context.

The general project freshness/pruning audit was already overdue and remains explicitly due in the cursor. This migration verifies memory consistency, not every application statement or platform. No unrelated task is implemented or declared complete. No releases, pushes, tags or publication occur.
