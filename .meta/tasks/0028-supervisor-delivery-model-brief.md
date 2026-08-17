# T-0028 Brief: Reconsider The `rt` Bind Mount And Supervisor Delivery Model

## Identity And Source

- Task ID: T-0028
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User direction while finalizing T-0027
- Source reference and date: T-0027 finalization, 2026-08-13
- Parent or split task IDs: None. Depends on T-0027.

## Goal

Re-evaluate how the supervisor, control script, command wrapper, and startup hook
scripts reach the container, now that the constraint which forced the current design is
gone. Decide deliberately whether to keep the read-only `rt` bind mount, bake assets
into the image, or adopt a hybrid — and record the outcome as a decision.

This task is a **design/decision task**. It may legitimately conclude "keep the current
model, with reasons recorded" — that is a successful outcome, not a failure.

## Background

T-0024 D1 chose to deliver the supervisor as POSIX `sh` scripts written to
`.dcc/<profile>.rt/` on the host and bind-mounted read-only at
`/usr/local/share/dcc/rt`. The design document names the deciding constraint
explicitly:

> **Blocker found in research:** `uses_fast_path` (`src/build.rs`) pulls the user's
> image and `docker tag`s it without running `build_dcc_stage`, so `generated_assets()`
> never reach the image. A supervisor baked into the image would be **absent from every
> fast-path container**.

**T-0027 removes the fast path.** Once every image is built through the dcc build stage,
that blocker no longer exists — baking assets into the image becomes viable for the
first time. Two further constraints in the same decision were also downstream of the
fast path: the inability to install packages (which T-0025 D1 worked around with a
dependency-free FIFO handshake) and the "works identically on both paths" requirement,
which becomes vacuous when there is only one path.

The T-0025 r3 design deliberately did **not** revisit this. It retained the `rt` mount
and recorded two independent reasons that survive the fast path's removal:

1. Read-only delivery keeps supervisor scripts non-tamperable from inside the container.
2. A `dcc` upgrade fixes the supervisor without requiring an image rebuild.

Reason 2 is the substantial one, and it cuts against baking. This task exists to weigh
it properly rather than let the current model persist purely by inertia.

## Design Questions

The task should produce an explicit answer to each.

**Q1 — Does the version-skew argument survive scrutiny?** If supervisor scripts are
baked into the image, an image built by dcc `0.1.0` carries a `0.1.0` supervisor, and a
`0.2.0` CLI drives it. What breaks? Note that `version.rs` already warns on a
`dcc.version` mismatch and tells the user to rebuild, and after T-0027 *every* image
carries that stamp. Is the existing warning sufficient, or does the host↔supervisor
contract need a compatibility check of its own?

**Q2 — What is the actual tamper surface?** The `rt` mount is read-only, and
`/usr/local/share/dcc` is a T-0021 Tier-1 reserved subtree so users cannot target it with
`customizations.dcc.state`. Baked-in image assets are *also* not writable by a non-root
container user, but a root container user could overwrite them. Does that difference
matter given the T-0024 threat posture (failures that cannot escape the container are
tolerated; remediation is `dcc stop --kill`)? Check
`.meta/threat-models/0004-dcc-runtime.md` before answering.

**Q3 — Do startup hook scripts want the same answer as the supervisor?** They differ in
an important way: the supervisor changes only when `dcc` is upgraded, but hook scripts
change whenever `devcontainer.json` changes. Baking hooks would mean an image rebuild per
hook edit, which is clearly wrong. The T-0025 design already notes this. So the honest
answer may be split — bake the stable scripts, mount the volatile ones — which raises
Q4.

**Q4 — Is a split model actually simpler?** If hook scripts still need a host-side
directory and a bind mount, then baking the supervisor removes no machinery: the mount,
the `RtDir` type, and `materialize()` all still exist. The simplification argument for
baking may be largely illusory. Quantify what would actually be deleted before
recommending it.

**Q5 — Does baking unlock anything new?** With assets in the image, dcc could install
packages the supervisor depends on (`inotify-tools`, a static helper binary), enabling
mechanisms currently ruled out. T-0025 D1 rejected inotify partly because the fast path
forbade installing anything, but also concluded the FIFO handshake was *better* than
inotify on its own merits (zero dependencies). Is there a concrete capability that is
currently blocked and actually wanted? If not, this is a hypothetical benefit and should
be weighted accordingly.

**Q6 — Cold-start cost.** `materialize()` writes four-plus files to the host on every
runtime launch. Baked assets would remove that per-launch I/O. Measure whether it is
detectable against the `docker run` cost before treating it as a benefit.

## Scope

In scope:

- A decision record answering Q1–Q6 and selecting a delivery model.
- If the decision is to change the model: a follow-up implementation task, split out
  rather than executed inside this one.
- If the decision is to keep the `rt` mount: update `.meta/tasks/0024-r1-supervisor-design.md`
  (or a superseding note) so its stated rationale no longer rests on the removed fast
  path, and record the surviving reasons.

Out of scope:

- Implementing a delivery-model change. That is deliberately a separate task, so the
  decision is made on its merits and not under implementation pressure.
- Changing the supervisor's behavior, the startup handshake, or the lifecycle contract.
  This is about *delivery*, not semantics.
- Reopening the POSIX `sh` choice. Language is a separate axis; a static binary could be
  revisited independently, and only if a concrete need appears.

## Users And Workflows

| User/Actor | Workflow | Expected Change |
| --- | --- | --- |
| Developer | Any runtime command | None. This task decides; any behavior change ships in a follow-up |
| Developer who upgrades `dcc` | Runtime command against an image built by the old version | Depends on Q1 — the central user-facing question |
| Maintainer | Changing the supervisor | Understands whether a supervisor fix requires users to rebuild images |

## Acceptance Criteria

- [ ] A decision record exists answering Q1–Q6, each with a recorded rationale.
- [ ] The decision names the selected delivery model and its consequences for supervisor
      scripts and hook scripts separately.
- [ ] If the model changes, a follow-up implementation task is cataloged with its own
      brief; this task does not implement it.
- [ ] If the model is kept, T-0024 D1's rationale is corrected so it no longer cites the
      fast path as a live constraint.
- [ ] The threat model is consulted and either confirmed still accurate or updated.

## Constraints

- Must not weaken the T-0021 reserved-path protection of `/usr/local/share/dcc`.
- Must not reintroduce host-backed *writable* runtime control state — that was the
  T-0023 exposure that T-0024 was opened to eliminate.
- Any conclusion must hold for both durable and one-shot containers and for the
  build-preparation container, which shares the same entrypoint.

## Workflow Route Rationale

- Cataloged route and risk: Design / Medium.
- Why this route: The deliverable is a decision, not code. The analysis spans the
  supervisor, threat model, version-skew behavior, and hook delivery.
- Why this risk gate: Not High, because the task changes nothing on its own and its
  worst outcome is a recorded "keep current model". Not Low, because a wrong conclusion
  would push a later change that could break cold start for every runtime command or
  silently pair a new CLI with an old supervisor.
- Upstream artifacts required: `.meta/tasks/0024-r1-supervisor-design.md` (D1),
  `.meta/tasks/0025-r1-startup-handshake-design.md` (D0, D2),
  `.meta/threat-models/0004-dcc-runtime.md`, T-0027's outcome.
- Escalation trigger: If the analysis concludes that baking requires users to rebuild
  images to receive a supervisor bugfix, escalate before selecting it — that is a
  product-level support decision, not an implementation detail.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Decision made on aesthetics ("baking is cleaner") rather than evidence | Churn with no benefit, or a regression | Q4 requires quantifying what is actually deleted; Q6 requires measuring, not assuming |
| Version skew between a new CLI and an old baked supervisor | Silent protocol mismatch at runtime | Q1 is the gating question; the existing `dcc.version` warning is the starting point |
| Splitting supervisor and hook delivery adds a second mechanism | More surface, not less | Q3/Q4 force the split to justify itself against a single uniform mechanism |
| Task drifts into implementation | Decision made under pressure to justify work already done | Implementation is explicitly out of scope and split to a follow-up |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| T-0027 removes the fast path and every image gains a `dcc.version` stamp | High | T-0027 acceptance criteria; this task depends on it |
| Hook scripts must stay host-delivered because they change with config | High | Stated in T-0025 D2; re-verify in Q3 |
| The `rt` mount's read-only property is not load-bearing for the threat model | Medium | Q2 explicitly checks this against `.meta/threat-models/0004-dcc-runtime.md` |

## Verification Plan

- This task produces a decision, so verification is review-based rather than test-based.
- Documentation checks: the decision record answers every question; no surviving
  document cites the fast path as a reason for the current delivery model.
- If a follow-up implementation task is created, it carries its own verification plan.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r1 | 2026-08-13 | User direction | Initial intake | — | — |

## Done When

- Q1–Q6 are answered in a decision record with recorded rationale.
- A delivery model is selected, or the current one is deliberately reaffirmed.
- Stale rationale citing the fast path is corrected wherever it survives.
- Any resulting implementation work is cataloged as its own task.
