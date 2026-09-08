# T-0084: Profile Running Indicator Design

## Identity And Source

- Task ID: T-0084
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User instruction
- Source reference and date: Product-owner design request, 2026-09-08
- Parent or split task IDs: T-0057 established the current `profile list` contract.

## Goal

Extend `dcc profile list` so a user can see which discovered profiles currently have
an active dcc runtime container, without parsing profile configurations, issuing one
Docker command per profile, or misreporting Docker failures as "not running."

## Current Boundaries

- `profile::discover_profiles` finds direct `.devcontainer/*.json` profiles and sorts
  them without loading their contents.
- `ContainerId::new(workspace, profile)` derives the stable identity shared by the
  profile image and all of its dcc-created containers.
- Runtime and build-preparation containers currently both carry
  `dcc.container_id=<container-id>`.
- Runtime containers may use a configured Docker-visible name, and one-shot runtime
  containers are still valid running profile containers while their command is active.
- `docker::running_container_name_by_id` performs an exact per-profile label query and
  is appropriate for lifecycle commands, but calling it for every listed profile would
  make listing O(number of profiles) in Docker subprocesses. It also cannot distinguish
  a runtime container from the transient build-preparation container.
- T-0057 deliberately made profile discovery usable without Docker. Running state
  cannot be known without a successful container-engine query, so the new output needs
  a third `unknown` state rather than converting query failures into `false`.

Docker documents that `docker ps` lists running containers by default, supports
filtering on the presence of a label, and exposes individual label values and names to
Go output templates:
<https://docs.docker.com/reference/cli/docker/container/ls>.

## User-Visible Contract

The command attempts one best-effort, read-only Docker status snapshot after filesystem
discovery. It does not load, validate, or merge profile configuration.

### Text

Append an independent `(running)` marker only to profiles known to have a runtime
container. Preserve the existing name and default marker exactly:

```text
ci (running)
devcontainer (default)
```

When the default profile is running, use composable markers rather than changing the
meaning of the existing one:

```text
devcontainer (default) (running)
```

Profiles known not to be running retain their current text. If Docker status is
unavailable, retain those profile lines, emit one concise warning on stderr, and emit
no running markers. The warning must state that status is unknown, not imply that no
containers are running. Detailed process failure context belongs behind `--debug` so
normal output stays concise and JSON stdout remains parseable.

### JSON

Add `running` after the existing stable fields:

```json
{
  "profiles": [
    {
      "name": "ci",
      "config": ".devcontainer/ci.json",
      "default": false,
      "running": true
    },
    {
      "name": "devcontainer",
      "config": ".devcontainer/devcontainer.json",
      "default": true,
      "running": false
    }
  ]
}
```

The field is tri-state:

| Value | Meaning |
| --- | --- |
| `true` | The Docker snapshot contained at least one runtime container for this profile ID. |
| `false` | Docker returned a valid snapshot and contained no runtime container for this profile ID. |
| `null` | Docker could not provide a trustworthy snapshot or its output could not be parsed. |

Use a serialized `Option<bool>` so the unknown state is explicit and the field is never
silently omitted. Adding the field intentionally revises T-0057's exact JSON record
contract; consumers should use JSON rather than parsing the text suffixes.

If no profiles are discovered, return the existing empty text or `{"profiles":[]}`
result without invoking Docker because no status can be displayed.

## Runtime Classification

"Running" means a dcc **runtime** container is present in the point-in-time Docker
snapshot. It includes both durable containers and one-shot containers while they are
alive. It excludes stopped containers, state-hydration containers, and the transient
build-preparation container used by `dcc build`.

Add a role label to both dcc container construction paths:

```text
dcc.container_role=runtime
dcc.container_role=build-prep
```

This is host-side metadata only. It does not alter the supervisor protocol or require a
version-compatibility change.

For containers created by an older dcc that have `dcc.container_id` but no role label,
apply this compatibility rule:

1. A role of `runtime` counts as running.
2. A role of `build-prep` does not count.
3. With no role, a container named exactly `<container-id>-build-prep` is the known
   legacy build-preparation container and does not count; any other name counts as a
   legacy runtime container.
4. An unknown non-empty role makes that profile's state unknown unless another record
   already proves a runtime is running. Emit one concise warning and include the role
   detail only in debug output.

The only compatibility ambiguity is an old runtime container deliberately configured
with the exact generated build-preparation name. New containers are unambiguous because
the role label wins over the name fallback.

## Docker Query And Data Flow

Add a Docker helper that obtains all running dcc container records in one subprocess:

```text
docker ps \
  --filter label=dcc.container_id \
  --format '{{.Label "dcc.container_id"}}\t{{.Label "dcc.container_role"}}\t{{.Names}}'
```

The default `docker ps` scope intentionally excludes stopped containers. Parse each
record into a small internal type containing container ID, optional role, and Docker
name. Require exactly three tab-separated fields and reject an empty container ID;
Docker container names and dcc-generated IDs/roles cannot contain tabs. A malformed
record makes the whole snapshot unavailable instead of producing partial false
negatives.

The list flow becomes:

1. Discover and sort profiles with the existing filesystem-only function.
2. Return immediately if discovery is empty.
3. Compute each profile's `ContainerId` without loading its config.
4. Await one `docker::running_dcc_containers()` call.
5. Classify records by role and reduce each relevant ID with the precedence
   `running > unknown > not running`: a current or legacy runtime proves `Some(true)`,
   only known build-prep records yield `Some(false)`, and an otherwise unrecognized role
   yields `None`.
6. On spawn, daemon, non-zero-exit, UTF-8/record-shape, or other query failure, warn once
   and enrich every profile with `None`.
7. Render the already-sorted enriched records in text or JSON.

`profile::list_profiles` therefore becomes async, and both early and exhaustive match
dispatches in `main.rs` await it. The existing lifecycle-specific
`running_container_name_by_id` remains unchanged: it returns a name and enforces a
single-container invariant, while listing only needs a batched point-in-time boolean.

## Alternatives Considered

| Option | Decision | Reason |
| --- | --- | --- |
| Call `running_container_name_by_id` for each profile | Reject | Correctly matches stable IDs but creates N Docker subprocesses and counts build-prep containers. |
| Match Docker-visible container names | Reject | Runtime names may come from config, and listing must not parse configs. One-shot and durable containers already share the stable label identity. |
| Make Docker failure fatal | Reject | Regresses the useful offline discovery behavior and conflates profile discovery with status availability. |
| Treat Docker failure as `running: false` | Reject | Produces a dangerous false negative when Docker is missing, stopped, remote, or inaccessible. |
| Add an opt-in `--status` flag | Reject for the initial implementation | Preserves byte-for-byte old output but does not satisfy the default discoverability goal; tri-state best effort keeps the default useful without making Docker mandatory. |
| Count every `dcc.container_id` label | Reject | Concurrent `dcc build` preparation would briefly look like an active runtime environment. |

## Implementation Slices

1. **Container metadata and query seam**
   - Add shared label/role constants.
   - Label runtime and build-preparation `docker run` argument vectors.
   - Add pure query-argument and output-parsing helpers in `docker.rs`.
2. **Profile enrichment and rendering**
   - Keep discovery pure, add the tri-state field, classify current and legacy records,
     make listing async, and update debug/warning behavior.
3. **Contract coverage and documentation**
   - Extend CLI/fake-Docker tests, add a live lifecycle smoke, and update README,
     `docs/index.md`, architecture, and the new implementation task record. Preserve
     T-0057's completed brief and quality record as historical evidence of the prior
     contract.

These slices should ship as one implementation task because the role label and output
contract are not independently user-visible or useful.

## Automated Test Design

### Pure/unit coverage

- Exact Docker argv uses one `ps`, the label-presence filter, and the three-field format.
- Parser accepts runtime, build-prep, and missing-role records; rejects missing fields,
  extra fields, and empty IDs without returning a partial snapshot.
- Classifier includes current runtime and legacy runtime records, excludes current and
  legacy build-prep records, tolerates duplicate runtime IDs, and maps an unknown role
  to unknown unless another record proves the profile is running.
- Text composition covers running-only, default-only, both markers, control-character
  escaping, and known-not-running output.

### CLI/fake-Docker coverage

- Multiple sorted profiles result in exactly one Docker call and correct running markers.
- JSON emits exact `true` and `false` values in stable field order.
- Missing Docker and a non-zero `docker ps` both preserve successful discovery, warn
  once on stderr, and emit `running: null` in JSON.
- Malformed Docker output follows the same all-unknown policy.
- An empty profile set performs no Docker call and retains the existing exact output.
- Global `--profile`, `--format`, and `--debug` placement continues to work without
  resolving or loading the selected profile.
- A build-preparation record for a discovered profile does not produce a running marker.

### Live Docker coverage

Add one ignored Docker smoke that builds or reuses the fixture profile image, verifies
`running: false`, starts the durable profile, verifies `running: true`, stops it, and
verifies `false` again. Existing lifecycle smokes continue to establish one-shot reuse;
the fake boundary test covers build-prep exclusion deterministically.

The primary counterfactual tests are the text/JSON assertions and Docker-call log: they
fail on the current implementation because it emits no running field or marker and does
not invoke Docker. A role-classification mutation that counts build-prep must fail its
dedicated test.

## Acceptance Criteria

- [ ] Text output appends `(running)` exactly for discovered profiles with a current
      runtime container and composes it after `(default)`.
- [ ] JSON records contain stable `running: true`, `false`, or `null` values.
- [ ] Durable and live one-shot runtime containers count; build-preparation and stopped
      containers do not.
- [ ] Listing performs at most one Docker subprocess regardless of profile count and no
      Docker subprocess for an empty profile list.
- [ ] Missing, inaccessible, failed, or malformed Docker status produces unknown state,
      one warning, successful profile discovery, and no false stopped claim.
- [ ] Direct-profile discovery, ordering, escaping, empty output, subdirectory behavior,
      and bypass of selected-profile/config resolution remain intact.
- [ ] Runtime/build-prep role labels and the legacy fallback are covered at the Docker
      argument and classification boundaries.
- [ ] User and architecture documentation describe the snapshot, role boundary,
      tri-state JSON, and offline degradation accurately.

## Risks And Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Docker is unavailable | Listing falsely says profiles are stopped or fails entirely. | Use `null`/one warning and preserve successful discovery. |
| Build prep shares the stable profile ID | A build looks like an interactive runtime. | Add role labels and retain an exact-name fallback for old containers. |
| Many profiles cause slow listing | Latency grows linearly with profile count. | Query all dcc-labeled running containers once and intersect in memory. |
| Status changes after the query | Display becomes stale immediately. | Document it as a point-in-time snapshot; do not imply a lock or lifecycle guarantee. |
| Text parsers depend on old lines | Running profiles gain a suffix. | Keep stopped lines and the default marker unchanged; direct integrations to JSON. |
| JSON consumers reject new fields | Additive field breaks overly strict consumers. | Document the intentional contract revision and cover exact new output. |
| Legacy name fallback is ambiguous | One narrowly named old runtime may be missed. | Role wins for all new containers; record the compatibility limitation. |

## Verification Plan For Implementation

- Focused: profile unit tests, CLI flag tests, and Docker-boundary tests.
- Required full checks: `cargo fmt --check`, `cargo check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo build`.
- Environment-owned end to end: the ignored Docker lifecycle smoke in Docker-capable CI.
- Manual: text/JSON success, Docker-unavailable degradation, debug stderr, help/docs,
  exact Docker argv, and `git diff --check`.
- Review: correctness first, then compatibility of the output contract, build-prep
  exclusion, error truthfulness, subprocess count, and documentation consistency.

## Done When

The design fixes the user-visible semantics, exact text/JSON/error contracts, runtime
classification and legacy behavior, one-query implementation seam, regression matrix,
risks, documentation updates, and implementation acceptance criteria without changing
production code.
