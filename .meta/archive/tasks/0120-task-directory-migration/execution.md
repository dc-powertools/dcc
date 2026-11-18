# Execution and verification

## Authority, readiness and boundary

T-0120@r2: user execution request on 2026-11-18 supersedes r1's capture-only disposition. Initiative / Medium. Ready: agreed design and acceptance are explicit, the baseline worktree is clean, and the complete catalog/supporting inventory has been captured against baseline Git revision 5ffbb1d. No application behavior changes are authorized. Rollback is the baseline Git revision; do not reset or overwrite unrelated work.

Required checks declared before cutover: compare every task field against the baseline; account for every artifact; validate both index coverages, unique identities, expanded dependency ranges/cycles, current/terminal placement, links/affected anchors/literal references, cursor and safety boundaries; review framework/template consistency and byte/line budgets; independent preservation review plus cold-start/current recovery and targeted historical lookup. Application runtime tests are not applicable to documentation and one source-comment reference correction.

## Plan and worker roster

1. Inventory and record explicit ownership/path mapping (complete).
2. Align reusable policy/templates and startup detection (framework worker).
3. Create current bundles first, preserve current task state, then bundle historical and catalog-only tasks (root).
4. Reconcile operative decisions and cross-cutting security into project owners (requirements worker).
5. Integrate references, validate integrity, perform independent recovery/review, and resolve findings (root + reviewer).
6. Record final results, archive T-0120, rerun final checks and commit locally (root).

Workers share the worktree with explicit disjoint ownership:

- `framework`: `.meta/meta/**`, root AGENTS and optional harness adapters; integrate only output bound to T-0120@r2.
- `requirements`: `.meta/project/**`; current requirements and historical origin mapping, no moves.
- `integrity_review`: read-only baseline preservation assessment, final independent diff/recovery review.
- Root: task records, bundles, indexes, cursor, mapping, reference integration, verification and final commit.

Restart only needed missing work after reconciling the roster and shared diff. Preserve unknown changes. No worker may commit or alter another worker's owner files.

## Cutover checkpoint

All 126 task READMEs created. All 106 supporting artifacts moved using the explicit map. Five unrelated nonterminal tasks retain exact state and remain current. Primary task remains T-0108, Scheduling Running, global pause source None. Historical terminal tasks are archived. T-0120 was archived after all required checks and independent review completed; final coverage is five current and 121 archived task bundles.

## Threat review

Assets are task authority, approval boundaries, applicable product/security requirements and historical evidence. Risks include archive text becoming current authority, lost catalog-only state, numeric-prefix ownership errors, accidental completion upgrades, and broadened tool authorization. Mitigations: canonical task READMEs and explicit provenance; current-doc reconciliation with supersession/qualification distinctions; reviewed mapping; exact baseline comparisons; one-owner shared evidence; both-namespace identity checks; independent recovery and safety-boundary review. Historical research and untrusted input never supply tool authority. This migration does not expand any stop, launch, publication or privilege boundary.

## Framework change rationale

Disposition: Adopt. Decisive evidence is the user's explicit T-0120 design and execution instruction (level 4). Before: a 102 KB startup catalog owns all lifecycle state and a shared sequential allocator coordinates independent branches poorly. After: per-task READMEs own state, current discovery and history are separate, and random IDs are checked for collision at allocation/integration. Significant requirements live in applicable current docs, with historical rationale task-local. Keep policy Markdown-only and retain all approval, review, source-trust and quality gates. Review at the next hygiene pass: cold-start context size, duplicate IDs, link integrity and successful requirement lookup; revise ambiguous wording rather than restore duplicated lifecycle owners.

## Results

All planned steps complete. Final integrity check passed: 126 canonical task records, 106 preserved supporting artifacts, 403 local Markdown links and affected anchors, expanded dependency DAG, unique identities, complete index coverage and correct placement; zero errors or anchor warnings. Final current index: 5 tasks, 11 lines/1,140 bytes. Final cursor: 60 lines/2,904 bytes. Root AGENTS: 17 lines/919 bytes. Core Markdown-only, process budgets and comment-only source comparison pass. Independent review approved after resolved findings; fresh-agent recovery and targeted historical lookup passed. Whitespace check passed. No application tests run because executable behavior is unchanged. See verification.md for method and limitations.
