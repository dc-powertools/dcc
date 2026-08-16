# 0004: Embed The Supervisor In The Image; Keep Startup Hooks Host-Delivered

Status: Accepted

Date: 2026-08-13

Owners:

- T-0028 implementer

Supersedes:

- T-0024-R1 design decision D1 (delivery mechanism only; the POSIX `sh` language
  choice and the `/run/dcc` tmpfs state model in D2 are unchanged)

Superseded by:

- None

## Context

T-0024 D1 delivered the supervisor (`dcc-supervisor`), the control script (`dcc-ctl`),
and the command wrapper (`dcc-exec`) as POSIX `sh` scripts written to
`.dcc/<profile>.rt/` on the host and bind-mounted read-only at
`/usr/local/share/dcc/rt`. That design named its deciding constraint explicitly: the
image fast path (`uses_fast_path`) pulled and re-tagged the user's image without running
`build_dcc_stage`, so nothing dcc generated could reach the image.

T-0027 removed the fast path. Every `dcc build` now runs `build_base_image` +
`build_dcc_stage`, and every dcc-built image carries a `dcc.version` label. The
constraint that forced the bind mount is gone, so the delivery model was re-evaluated.

T-0028's brief posed six questions. The answers below reflect owner direction given
during the task, which narrowed several of them.

## Decision

**Bake the three supervisor scripts into the image. Keep startup hook scripts
host-delivered through the `rt` bind mount.** Add a semantic-version compatibility gate
so the CLI refuses to drive an image built by an incompatible dcc.

The split is not a matter of taste: it follows from a hard technical constraint on hook
substitution, established in Q3 below.

## Answers To The Design Questions

### Q1 — Version skew: handled by a semver compatibility gate, not by delivery model

Version skew is not a reason to avoid baking. The supervisor's version is tracked by the
existing `dcc.version` image label, which after T-0027 is present on every dcc-built
image. The remedy for skew is trivial and already documented: rebuild.

Today `version_warning` only ever *warns*. Baking the supervisor makes the host↔supervisor
protocol (the `dcc-ctl` verbs `mode`/`stop`/`stop-now`/`wait-ready`, the `dcc-exec`
registration contract, and the exit codes 252/253) part of the image rather than
something the host rewrites on every launch. A warning is too weak for a protocol
mismatch.

**Decision:** interpret `dcc.version` with semantic-versioning compatibility rules.

| Image vs CLI version | Behavior |
| --- | --- |
| Equal | Proceed |
| Differs in patch only | Proceed (compatible) |
| Differs in major or minor | **Refuse**, with an error naming the rebuild command |
| Label absent | **Refuse** (only pre-T-0027 or non-dcc images lack it) |

Patch releases are therefore constrained: they must not change the
host↔supervisor protocol. Any protocol change requires at least a minor bump. This is
recorded as a maintenance rule, not merely a convention, because the compatibility gate
depends on it.

This replaces the current unconditional warning for runtime commands. `dcc build` is
exempt — it is the command that fixes the problem.

### Q2 — Tamper surface: not an attack path we defend

The read-only bind mount prevented container-side code from modifying the supervisor.
Baked assets are writable by a root container user. Per owner direction and consistent
with the T-0024 posture recorded in `readme/threat-models/0004-dcc-runtime.md` — failures
that cannot escape the container are tolerated, remediation is `dcc stop --kill` — this
is explicitly **not** an attack path dcc defends against. A root user inside the
container can already subvert its own lifecycle by other means.

The read-only property was therefore never load-bearing for the threat model. Dropping it
costs nothing we were relying on.

`/usr/local/share/dcc` remains a T-0021 Tier-1 reserved subtree, so
`customizations.dcc.state` still cannot target it. That protection is independent of
delivery model and is unchanged.

### Q3 — Hooks cannot be baked: `${localEnv:VAR}` is irreducibly host- and run-time

This is the load-bearing finding of the task, and it is a technical constraint rather
than a preference.

The two hook families are executed by *different mechanisms*:

- **Build-prep hooks** (`onCreateCommand`, `updateContentCommand`, `postCreateCommand`)
  are `docker exec`'d by the host. `run_planned_hooks` (`src/build.rs`) substitutes the
  command and calls `lifecycle::run_in_container`, which execs the resolved argv
  directly. The `.sh` files that `generated_assets()` bakes to
  `/usr/local/share/dcc/hooks/build-prep/` are **never executed** — they are vestigial.
  Substitution therefore always happens in the host process, against live host state.
- **Startup hooks** (`postStartCommand`) are executed *by path inside the container*:
  the supervisor runs `sh "$f"` over `--start-hooks <dir>`. The script text must
  therefore be **fully resolved before it enters the container**.

For baked startup hooks, resolution would have to happen at image build time. It cannot,
because of `${localEnv:VAR}`:

- `apply_substitutions` (`src/config/vars.rs`) applies `apply_substitution` to the entire
  `lifecycle` struct at config load, and `apply_substitution` resolves `${localEnv:VAR}`
  via `std::env::var` — the environment of the *invoking* `dcc` process.
- That value legitimately differs between `dcc build` and a later `dcc exec`, and between
  two different users sharing one image. Baking it would freeze the builder's environment
  into the image, which is precisely wrong.
- The devcontainer spec permits `${localEnv:…}` in lifecycle commands, so this is a
  compatibility obligation, not an edge case.

The codebase already encodes exactly this rule. `containerEnv` values are baked into the
image at build time, and for that reason they are substituted with
`apply_container_env_substitution`, which deliberately **omits** `localEnv`. The comment
on that function states the principle directly. Fields that get baked may not read
`localEnv`; fields that are runtime-applied may. Startup hooks are runtime-applied today,
and moving them into the image would silently move them across that line.

The other substitution inputs would have been fine:

| Token | Build-time feasible? |
| --- | --- |
| `${localWorkspaceFolder}`, `${localCacheFolder}` | Yes — the container-side targets `/workspace` and `/cache` are constants |
| `${containerEnv:VAR}` | Largely — the image env is known at build time; `HOME`/`USER` are probeable, and after T-0026 the uid is resolved at build time |
| `${localEnv:VAR}` | **No** — reads the invoking user's host environment at run time |

So a single token class blocks baking. The alternatives were considered and rejected:

- **Bake hooks and drop `localEnv` support in `postStartCommand`.** A silent
  spec-compatibility regression that would break working configs. Rejected.
- **Bake hooks with `localEnv` deferred, resolved inside the container at run time.**
  This requires shipping a substitution engine into the container and passing the host
  env in, which reintroduces a host dependency by another name while adding a second
  substitution implementation to keep consistent with the Rust one. Strictly worse.
  Rejected.

Note that the owner's premise — that a `devcontainer.json` edit should require a rebuild
— is accepted and is *not* what keeps hooks on the host. Hook volatility is no longer
a reason. `${localEnv:VAR}` is.

### Q4 — "Simpler" was excluded from the evaluation

Per owner direction, relative simplicity and count-of-lines-deleted were not weighed.
The decision rests on the Q1 compatibility gate and the Q3 substitution constraint.

### Q5 — Not considered

Excluded from the evaluation per owner direction. Baking does make installing
supervisor dependencies possible in future, but no such need is claimed here and none is
acted on.

### Q6 — Not considered

Excluded from the evaluation per owner direction.

## Consequences

### Supervisor

- `dcc-supervisor`, `dcc-ctl`, and `dcc-exec` are emitted into the build context and
  `COPY`'d into the image, alongside the existing `.dcc-generated/` mechanism that
  already targets `/usr/local/share/dcc/`. They become image content, marked executable
  by the existing `find … -exec chmod +x` step.
- The `--entrypoint` path and the `dcc-ctl`/`dcc-exec` exec paths move from
  `/usr/local/share/dcc/rt/…` to their baked location.
- Because the scripts are baked, a change to `src/supervisor.rs` requires a rebuild to
  take effect. The `dcc.version` label already invalidates the build cache on a version
  bump (it is the first post-`FROM` instruction), so a released change is picked up by
  the next `dcc build`. During development of dcc itself, an unbumped local edit needs
  `dcc build --no-cache` or a version bump; this is a maintainer-facing cost and is
  accepted.
- The vestigial, never-executed build-prep `.sh` assets under
  `/usr/local/share/dcc/hooks/build-prep/` should be removed as separate cleanup; they
  are unrelated to this decision but were discovered while making it.

### Hooks

- `postStartCommand` scripts continue to be generated per launch into
  `.dcc/<profile>.rt/start-hooks/` and delivered by the bind mount, fully substituted
  host-side. `--start-hooks` semantics are unchanged.
- The `rt` bind mount therefore survives, but its contents shrink to only
  `start-hooks/`. `RtDir::materialize` no longer writes the three supervisor scripts;
  it only manages the `start-hooks/` directory.
- The mount can no longer be omitted-when-empty without care: the build-preparation
  container calls `materialize()` and passes no `--start-hooks`, so it may mount an empty
  directory or skip the mount entirely. Either is acceptable; the implementation task
  decides.

### Compatibility gate

- `version_warning` becomes a compatibility *decision* rather than a warning for runtime
  commands: patch-level drift proceeds, major/minor drift and a missing label refuse.
- A new maintenance rule: the host↔supervisor protocol may not change in a patch
  release.

## Follow-Up Work

This task decides only. Two implementation tasks are cataloged separately:

- **T-0029** — bake the supervisor scripts into the image, retarget the entrypoint and
  exec paths, shrink `RtDir` to `start-hooks/` only, and handle the build-prep container's
  mount.
- **T-0030** — replace the `dcc.version` warning with the semver compatibility gate
  (patch-compatible, major/minor and missing-label refuse), and record the
  no-protocol-change-in-a-patch-release rule in project standards.

T-0030 should land with or before T-0029: once the supervisor is baked, an incompatible
image is a protocol hazard rather than a cosmetic mismatch.

## Alternatives Considered

- **Keep everything on the `rt` bind mount** (the pre-T-0028 model). Its stated
  rationale rested on the fast path, which is gone, and on tamper-resistance, which Q2
  discards. Rejected.
- **Bake everything, including hooks.** Blocked by Q3's `${localEnv:VAR}` constraint
  without a spec-compatibility regression. Rejected.
- **Warn rather than refuse on version drift.** Too weak once the protocol lives in the
  image; a mismatched supervisor produces confusing failures at `dcc-ctl`/`dcc-exec`
  call sites rather than one clear error. Rejected.
