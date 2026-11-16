# T-0108 execution checkpoint

2026-11-16, r6, Initiative / High. User explicitly authorized coordinated agents,
Docker builds/tests and task-scoped local commits; no push/publication/backend change.
Parent remains open through coordinated catalog verification. macOS is unavailable;
the dcc completion gate and broad Feature rollout must remain incomplete.

## Baseline and ownership

Both worktrees initially clean. dcc `e600c5e605845644cac9244c927acb71d30c7e05`;
Features `068a82b0a1af42662abd7f6664e610af71757abf`.
Root owns integration, runtime/build/defaults, compatibility, docs and all task records.
Agents read both relevant entrypoints and framework guidance:

| Agent | Bounded assignment | Write ownership | Recovery |
| --- | --- | --- | --- |
| dcc_design | Contract investigation, then config/metadata implementation | src/config/{mod,resolve,vars,merge}.rs; src/features/{mod,validate}.rs; schemas/ | Inspect diff before restart; no runtime edits |
| verification | Independent coverage/platform investigation, fixtures and Docker regressions | tests/fixtures/persistence/; tests/persistence.rs; obsolete persistence cases in tests/docker_smoke.rs | Inspect existing results before resuming; later independent preservation review |
| codex_investigation | Current upstream layout/updater/sandbox investigation; necessary local pilot | Features pilot/codex/ and bounded pilot tests/docs | Keep published src catalog unchanged until dcc gate; report findings to root |

## Frozen shared contract

State strings and directory objects persist coherent roots. Legacy file state remains
supported with its atomic-replacement/EBUSY limitation; no new file-mount selection.
Exclusions require `type: "directory-with-exclusions"` and a nonempty `exclude` list
of `{path: "literal/child", target: "/absolute/image/location"}`. Old dcc rejects
the new type. Targets resolve container-side variables, stay outside every persistent
root, and must be installer-prepared directories. Literal paths have no variables,
globs, dot/empty components or overlapping selections. Conflicting duplicates fail.
Feature installers relocate in their installation layer and provide writable targets;
dcc verifies and prepares links after mounting persistence, before hooks, even when
user lifecycle hooks are skipped. Existing real directories and incorrect links are
renamed to recoverable siblings, never deleted. Refuse symlinked ancestors.

Defaults initialize only an entirely absent persistent root from the selected image.
Existing empty/populated roots survive untouched. No durable content traversal,
hashing, image refresh or reconciliation. Old seed ledgers stay untouched and ignored.
`--reseed-state` fails with migration guidance. dcc 0.3.0 gates old images/controllers.

Codex packages is installed payload; tmp is qualified as an exclusion. Upstream
rejects symlinked app-server-control, which remains persistent.
Preserve .tmp (marketplace user content) and mixed app-server-daemon preferences,
loaded-thread recovery and other user data. Provisioning daemon payload in the image
may be necessary to avoid stale-log/bootstrap behavior after replacement.

## Plan and declared verification

1. Resolve contract and Codex layout (contract frozen above; investigation ongoing).
2. Deterministic synthetic local Features precede runtime integration.
3. Implement config/schema, absence-only defaults, safe exclusion links, local pilot.
4. Run fmt, clippy all targets, full cargo tests/build, new live Docker regressions,
   migration/recovery, concurrency, large-state no-scan checks and local Codex probes.
5. Independent migration/data-preservation and test-coverage review; correct findings.
6. Update documentation/quality evidence and record exact tested revisions. Only
   verified coherent task-scoped slices may be committed. No broad catalog migration
   until Linux and macOS/Desktop gates pass; no releases/publication authorized.

## Verified Linux slice

T-0109 is complete within Linux implementation scope; T-0108 remains open.
`src/seed.rs` and Docker seed-label plumbing are removed. New defaults are absence-only,
old ledgers ignored, 0.3 image/controller and refresh gates enforced. Target mount
masking and symlink aliases are refused in image and mounted views. Concurrent known
profile startup waits for its immutable winning container without replaying hooks;
nonroot durable promotion atomically replaces mode state to avoid Linux sticky-file
protections. Initial negative controls exposed both bugs; final regression passes.

Observed checks: fmt, clippy all targets with warnings denied, 572 unit +124 runnable
integration tests and build passed. Existing live Docker smoke: 33/33. New persistence
suite: 10/10 (71.20s). Large 8GiB sparse cache with unreadable/FIFO canaries rebuilt
in 3.919s at 17,408 KiB peak dcc RSS. No backend changes or user credentials accessed.
Independent agents reviewed migration, alias/mount safety, concurrency and coverage;
all blocking findings in the implemented Linux slice were corrected.

Local Codex 0.169.0 integration uses Features pilot/codex only, qualifying packages
and tmp exclusions. Image provisioning includes daemon payload. .tmp marketplaces,
app-server-control sockets and app-server-daemon preferences/recovery remain durable;
upstream handles stale process identity/socket state. Explicit installer version
selection 0.169→0.168→0.169 succeeds, pruning skips symlink roots by design. Sandbox
probe succeeds with seccomp unconfined only in disposable tests; default seccomp
blocks namespace setup in baseline and relocated layouts. No runtime security defaults
changed. Automatic updater recovery, authenticated remote control and cross-version
daemon migration remain unqualified; rebuild is the supported durable update/rollback.

Exact committed revision pairing and final integration evidence are recorded below
after local commits; all content was tested before commit and immutable tree/binary
hashes allow equivalence checks. No broad Feature rollout or publication.

## Exact integration revision pairing

- dcc implementation: `0331498d5fe4d86125ec692e9907426b3b71a59f` (0.3.0).
- Features unpublished pilot: `6193b82c5d5ed49678858261ec69d079ce40d279`.
- Both clean committed revisions were used for the final local-reference smoke.
- Frozen executable SHA256:
  `35bf36c0e808c50f86426a4df5b4529b9158832af4a758766f8f7e801b9f3b6e`.
- Built image:
  `sha256:5293f5feec835c314ddc711513646f46961d46fd76d06d9a099e922aa2e2afd3`.

Commands, from `/tmp/dcc-0108-local-integration` using a task-local sudo Docker wrapper:
`bin/dcc-final build`; `bin/dcc-final start`; `bin/dcc-final exec sh -ec ...`
(pilot 10/10 checks, dummy retained history/marketplace cache/settings, daemon
start/version/stop); `bin/dcc-final stop --now`. All passed. Earlier final-content
checks also passed Debian 2/2 and two full rebuild/recreation cycles resetting
package/tmp markers while retaining user state. Logs are `exact-revisions-*.log`
and `final-*.log` in that temporary directory; durable findings are summarized here.
Runtime stopped afterward. Source/pilot commits are separate, nothing pushed/published.

T-0109 and T-0110 are Done within their bounded Linux/local-pilot scopes. T-0108 is
Needs verification: provide a Docker Desktop/macOS runner and qualify filesystem,
ownership, atomic saves and SQLite; complete remaining Codex updater/daemon migration
checks; then migrate/audit remaining Features individually, Rust separately. The
supporting dcc release must precede dependent production Feature publication, and
publication still requires separate authorization. No broad migration started.
