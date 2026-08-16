# T-0032 Brief: Whether To Bake Startup Hooks Into The Image

## Identity And Source

- Task ID: T-0032
- Initial revision: r1
- Catalog: `readme/tasks/README.md`
- Accepted source: User direction during T-0028 closure
- Source reference and date: T-0028 closure, 2026-08-13
- Parent or split task IDs: None. Depends on T-0028.

## Goal

Decide whether `postStartCommand` hook scripts should be baked into the image alongside
the supervisor (T-0029), or remain host-generated and delivered through the `rt` bind mount
as T-0028 provisionally concluded. This task produces a decision; it does not implement.

T-0028 settled the supervisor (bake it) and provisionally settled hooks (keep them on the
mount), but deferred the hook question to this task for explicit owner review. The
provisional conclusion rests on a single technical constraint — `${localEnv:VAR}` — that
is documented below in full so the decision can be made on the evidence.

## Background

### What changed in T-0028

T-0024 D1 originally delivered *all* runtime scripts (supervisor + hooks) through a
read-only `rt` bind mount, because the image fast path skipped `build_dcc_stage` so
nothing dcc generated could reach the image. T-0027 removed the fast path, and T-0028
decided to **bake the supervisor** into the image (T-0029) while **keeping hooks on the
mount**. The supervisor-vs-hook split was not aesthetic; it follows from a hard
difference in how the two hook families are executed and substituted.

### The two hook families execute differently

| Family | Hooks | Executed by | Substitution site |
| --- | --- | --- | --- |
| Build-prep | `onCreateCommand`, `updateContentCommand`, `postCreateCommand` | The **host**, via `run_planned_hooks` → `lifecycle::run_in_container` → `docker exec` of the resolved argv | Host process, against live host state, at `dcc build` time |
| Startup | `postStartCommand` | The **supervisor inside the container**, via `sh "$f"` over `--start-hooks <dir>` | Must be **fully resolved before the script enters the container** |

This asymmetry is decisive. Build-prep hooks are never executed from their baked `.sh`
files — the host re-derives and substitutes the command on each run, so their baked
scripts are vestigial (cleanup tracked as T-0031). Startup hooks are executed *by path*,
so their text must be complete and resolved at the moment they are written.

### The constraint: `${localEnv:VAR}` is irreducibly run-time

`apply_substitutions` (`src/config/vars.rs`) applies `apply_substitution` to the entire
`lifecycle` struct at config load. `apply_substitution` resolves `${localEnv:VAR}` via
`std::env::var` — the environment of the **invoking `dcc` process**. That value:

- Legitimately differs between `dcc build` and a later `dcc exec` (different invocations).
- Legitimately differs between two users sharing one image.
- Is permitted by the devcontainer spec in lifecycle commands, so dropping it is a
  compatibility regression, not an edge case.

For baked startup hooks, resolution would have to happen at image build time, which
freezes the builder's environment into the image — precisely wrong for a token whose
purpose is the invoking user's environment.

The codebase already encodes this exact rule: `containerEnv` values *are* baked into the
image, and for that reason they are substituted with
`apply_container_env_substitution`, which deliberately **omits** `localEnv`. The comment
on that function states the principle. Fields that get baked may not read `localEnv`;
fields that are runtime-applied may. Startup hooks are runtime-applied today; baking them
silently moves them across that line.

### What would have been fine

The other substitution inputs are build-time-feasible:

| Token | Build-time feasible? |
| --- | --- |
| `${localWorkspaceFolder}`, `${localCacheFolder}` | Yes — container-side targets `/workspace`, `/cache` are constants |
| `${containerEnv:VAR}` | Largely — image env is known at build; `HOME`/`USER` probeable; after T-0026 the uid is resolved at build time |
| `${localEnv:VAR}` | **No** — reads the invoking user's host environment at run time |

So a single token class blocks baking.

## Design Questions

**Q1 — Is `${localEnv:VAR}` in `postStartCommand` a real use case, or a theoretical one?**
The spec permits it and the code resolves it, but how often do real profiles use
`localEnv` inside a `postStartCommand` (as opposed to `containerEnv`, which is far more
common for startup-time configuration)? If it is rare in practice, the compatibility cost
of dropping it may be acceptable; if it is common, it is not.

**Q2 — Is dropping `${localEnv:…}` support in `postStartCommand` an acceptable
spec-compatibility regression?** This is the core trade. Baking requires either dropping
`localEnv` from startup hooks (a documented deviation from the spec) or deferring
`localEnv` resolution into the container (a second substitution engine + host env
pass-through, which reintroduces a host dependency by another name). The decision is
whether the uniformity benefit of baking outweighs the compatibility cost.

**Q3 — Does baking hooks actually remove the `rt` mount?** If hooks are baked, the
mount, `RtDir`, and `materialize()` could be deleted entirely — *if* nothing else uses
them. Verify whether any other mechanism (e.g. `--skip-lifecycle` clearing, the
build-prep container's empty-mount handling) depends on the mount existing. If the mount
survives anyway, baking hooks adds a second delivery mechanism for no simplification —
the same objection T-0028 Q4 raised (and that task excluded "simpler" from its
evaluation; this task should not, because removing the mount is the *point* of baking
hooks).

**Q4 — Is there a middle ground?** A hook script could be baked with `${localEnv:…}`
left as a literal token and resolved by a tiny in-container substitution step at startup
(e.g. the supervisor `envsubst`s the script before running it). This keeps `localEnv`
working without a full second engine, but it adds a container-side substitution step and
a dependency on `envsubst`/a shell function, and it must stay bit-compatible with the
Rust `apply_substitution`. Whether this is simpler or more complex than the mount is the
question.

## Scope

In scope:

- A decision answering Q1–Q4 and selecting a hook delivery model.
- If the decision reverses T-0028's provisional conclusion (bake hooks), amend
  `readme/decisions/0004-embed-supervisor-in-image.md` Q3 and update T-0029's scope to
  include baking hooks (and possibly deleting the `rt` mount entirely).
- If the decision affirms T-0028's conclusion (keep hooks on the mount), record the
  rationale so the question does not recur.

Out of scope:

- Implementing any change. T-0029 implements the supervisor baking; a hook change, if
  any, is folded into T-0029 or a sibling task by this task's decision.
- Changing `postAttachCommand` delivery (it stays host-side per T-0025 D6 regardless,
  since it runs per-attach, not per-container).
- Reopening the supervisor decision (T-0028 settled it; this task is hooks-only).

## Users And Workflows

| User/Actor | Workflow | Expected Change |
| --- | --- | --- |
| Developer using `${localEnv:…}` in `postStartCommand` | `dcc exec` | Depends on Q2: if `localEnv` is dropped, their hook resolves the builder's env, not theirs |
| Developer not using `localEnv` in hooks | Any | None — their hooks bake and run identically |
| Maintainer | Editing a hook template | Understands whether a hook edit needs a rebuild or a relaunch |

## Acceptance Criteria

- [ ] A decision answers Q1–Q4 with recorded rationale.
- [ ] The decision names the selected hook delivery model and its effect on the `rt`
      mount (kept, shrunk, or deleted).
- [ ] If the decision reverses T-0028's provisional hook conclusion, decision 0004 Q3
      is amended and T-0029's scope is updated.
- [ ] If `localEnv` support in `postStartCommand` is dropped, the deviation is
      documented in README and project standards as a known spec incompatibility.

## Constraints

- Must not silently break configs that use `${localEnv:…}` in `postStartCommand`
  without a documented decision.
- Any in-container substitution must stay bit-compatible with the Rust
  `apply_substitution` semantics, including the undefined-variable error behavior.
- Must hold for both durable and one-shot containers and for the build-preparation
  container (which calls `materialize()` but passes no `--start-hooks`).

## Workflow Route Rationale

- Cataloged route and risk: Design / Medium.
- Why this route: The deliverable is a decision with a spec-compatibility dimension.
- Why this risk gate: Not High, because the task changes nothing on its own. Not Low,
  because a wrong call silently breaks a spec-permitted pattern or leaves a second
  delivery mechanism in place for no benefit.
- Upstream artifacts required: `readme/decisions/0004-embed-supervisor-in-image.md`
  (Q3), `src/config/vars.rs` (`apply_substitution` / `apply_container_env_substitution`),
  `src/supervisor.rs` (`write_start_hooks`, `hook_script`).
- Escalation trigger: If the decision is to drop `${localEnv:…}` support in
  `postStartCommand`, escalate before recording it — that is a user-facing
  spec-compatibility decision, not an implementation detail.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| `localEnv` in `postStartCommand` is more common than assumed | Baked hooks silently use the builder's env | Q1 surveys real usage before deciding |
| Baking hooks leaves the mount in place anyway | Second mechanism, no simplification | Q3 verifies the mount can actually be deleted |
| In-container substitution drifts from Rust semantics | Hooks resolve differently than other fields | Q4 requires bit-compatibility or rejects the middle ground |
| Decision reverses T-0028 and invalidates recorded rationale | Stale docs | Acceptance criteria require amending decision 0004 and T-0029 |

## Assumptions

| Assumption | Confidence | Validation |
| --- | --- | --- |
| `${localEnv:VAR}` is the only build-time blocker for hooks | High | Traced in T-0028 Q3; other tokens are constants or build-time knowable |
| `postAttachCommand` stays host-side regardless | High | T-0025 D6; per-attach, not per-container |
| The `rt` mount is used only by hooks after T-0029 bakes the supervisor | Medium | Q3 verifies |

## Verification Plan

- This task produces a decision, so verification is review-based.
- Documentation checks: the decision answers every question; decision 0004 and T-0029
  are consistent with the outcome; any spec deviation is documented.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r1 | 2026-08-13 | User direction | Initial intake | T-0028 deferred the hook question here | — |

## Done When

- Q1–Q4 are answered with recorded rationale.
- A hook delivery model is selected, and its effect on the `rt` mount is stated.
- Decision 0004 and T-0029 are consistent with the outcome.
- Any spec deviation is documented.
