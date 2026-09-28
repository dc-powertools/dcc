# T-0096: Container-Managed Relay — Implementation Design

Design revision: r4, 2026-09-28. Design only; no product implementation or runtime
verification. User authority includes the stop-before-build preflight and explicit
"Just use the socat behavior with -t 2" selection. Product policies remain owned by
[decision 0008](../decisions/0008-relay-in-development-container.md) and
[decision 0009](../decisions/0009-warn-and-defer-running-config-changes.md).

## Outcome And Scope

Concurrent sessions reuse container-owned forwarding without acquiring host ports.
The existing shell PID 1 owns relay services inside the development container.
No sidecar, host daemon, custom relay binary, runtime package installation, or live
configuration reconciliation. The new protocol ships across a minor-version boundary
(currently 0.2.0); this task does not bump versions, implement, or release it.

```text
host 127.0.0.1:4173 (and ::1 when available)
  -> Docker publication to container :20000
  -> socat child of the existing supervisor
  -> application 127.0.0.1:4173
```

The internal listener binds IPv4 wildcard. Docker-network peers can reach it under
normal network rules, the explicitly accepted simplicity tradeoff. Host publication
always names loopback. Application-only IPv6, host/none networking with forwarding,
and tunnels back from a remote Docker daemon are outside the supported contract.
Qualify localhost isolation on Docker Engine 28 or later (including Desktop's
Engine); refuse new forwarding launches on older daemons with upgrade guidance
because Docker documents an older localhost-publication exposure bug. Existing
container access and stop remain available. Custom routed/non-NAT networks are
outside normal bridge support and must not be reported as localhost-isolated.

## Configuration And Mapping

Add `customizations.dcc.relayPortRange: [20000, 20999]`, an inclusive pair. Both values
must be integers in 1024..65535 with start <= end. Omission uses the shown default;
an inherited child value replaces the whole pair. Document the range alongside
`forwardPorts`; it reserves only allocated ports, not the entire range.

Deduplicate and sort target ports, rejecting port zero. Assign the lowest unused
range ports in target order, skipping every declared target port. Range exhaustion
fails before Docker creation. No probing/remapping of application ports, random
allocation, or rewriting mappings on reuse. Record `{host_port, proxy_port,
target_port}` entries in the immutable launch snapshot; host and target ports match.
A proxy bind collision names both ports and advises choosing a different range and
recreating. An occupied host port remains a real startup error.

For each entry generate explicit Docker publication arguments for
`127.0.0.1:HOST:PROXY/tcp` and `[::1]:HOST:PROXY/tcp`. Reject host/none and network
namespace sharing when forwarding is configured. Retain existing runtime safety
checks; user-supplied publication arguments cannot override dcc's allocated mappings.
Do not infer that a changed current network setting should invalidate frozen reuse.
Reject `--publish-all` with forwarding, even with unsafe-runtime permission: it
could publish allocated proxy ports on wildcard host interfaces.

## Packaged Relay And Supervision

Use a baked wrapper that validates decimal ports and execs the selected, absolute
socat path with the equivalent of:

```sh
socat -t 2 TCP4-LISTEN:PROXY,bind=0.0.0.0,reuseaddr,fork TCP4:127.0.0.1:TARGET
```

No shell expressions, hostnames, arbitrary socat options, payload logging, `-T`, or
extra EOF timer. `fork` handles independent connections; application connection
failure ends that child only. The user selected socat's actual closing-wait behavior,
including resetting the wait during continued traffic, not a hard two-second deadline.
Errors and two-sided EOF may close earlier. A fully open connection has no added timer.

Provision when forwarding is configured at image build. Reuse a suitable installed
socat; otherwise install the distro's maintained `socat` package through the existing
apt/apk/dnf/yum build path. Validate TCP4 listen/connect, fork and `-t` capabilities.
Use supported distro packages, not downloaded executables or an unpatched upstream
version pin. Verify `setsid`, `readlink`, and the existing shell/text utilities too;
provision missing session support through the distro or fail with an actionable error.
The initial package qualification matrix is Debian/Ubuntu, Alpine, and Fedora-family
images. A later launch that first introduces forwarding to an image without these
capabilities requires an explicit rebuild. Empty-port images need no relay packages.

PID 1 launches one service runner per mapping, outside `/run/dcc/active`. Each runner
starts its listener in a separate session/process group via `setsid`, retaining its
PID and process identity. Disable shell job control; require the tested setsid path
to exec without an unexpected fork. Forked connection workers inherit the group.
Runner control state is private under `/run/dcc/relay`; never accept a PID/PGID from
a client or a snapshot. On listener failure, signal and reap that generation before
starting another. Group termination handles connection children, including those
left after a listener crash. Verify PID identity before delayed signals, and never
signal PID/group 0 or 1. Failed cleanup prevents further restart of that mapping.
Create the relay state directory privately (0700) before accepting user commands.

Readiness uses Linux procfs: match a LISTEN entry at the allocated IPv4 port to a
socket inode held by the tracked listener PID. A port-open test alone could mistake
another process for our relay. This also avoids connecting to the application as a
readiness probe. Check during the existing 200 ms polling cycle, with a 5-second
listener-start deadline. Missing/unreadable procfs fails readiness clearly; no false
success or fallback to app-connect probing. Use a small fixed parser with fixtures;
no `ss` dependency, log-message parsing, or general service-manager framework.

| State/event | Action |
| --- | --- |
| No mappings / build-prep role | Relay disabled; no service starts. |
| Initial bind succeeds | Publish per-port ready state. |
| Initial bind fails or readiness times out | Record failure and stop that generation; fail launching command. No initial bind retry/remap. |
| Listener exits after becoming ready | Clean its group; retry after 1 second. |
| Recovery | At most 3 restart attempts per mapping over this container lifetime; each must pass readiness. No timer reset or endless crash loop. |
| Attempts exhausted | Mark degraded; other mappings and user commands continue. |
| Stop begins | Disable restart immediately. Keep healthy forwarding while active user commands drain. |
| Final teardown | Terminate relay groups, allow up to 2 seconds for process cleanup, then kill remaining owned processes and reap before PID 1 exits. |

The final cleanup grace is separate from socat's connection closing wait. It does not
extend active-command draining or make infrastructure count as user work. Forced
Docker removal may bypass graceful cleanup; container process teardown is the final
boundary. Shutdown hooks keep their existing order before final relay termination.

## Readiness, Health, And Diagnostics Protocol

Keep startup hooks and forwarding status separate. Begin listener startup before
hooks; record its outcome, then run existing startup hooks even if forwarding failed.
`--skip-lifecycle` affects hooks, never the relay. A failed relay marks startup failure
and suppresses the one-shot orphan reap so the container remains for diagnostics.
Explicit stop and teardown after actual user commands remain available.

Extend the baked control scripts within the minor-version protocol boundary:

| Operation | Contract |
| --- | --- |
| `wait-ready` | New-launch caller waits for hooks and initial relay outcome; success requires both. Report port errors separately from hook errors. |
| `wait-hooks` | Existing bootstrap wait/status behavior, used by `dcc-exec`; later relay degradation never blocks a command. |
| `relay-status` | Bounded fixed-format state plus decimal ports/restart counts, no secret values or raw payload. |
| `snapshot` | Emit versioned JSON from one fixed read-only launch path; host calls through non-TTY `docker exec -u 0`. No caller-supplied path. |
| `verify-config HEX` | Validate one SHA-256 hex argument; compare with immutable launch fingerprint; return match/different distinctly from protocol failure. |

Use existing FIFO readiness discipline and atomic temporary-file/rename status
updates. No JSON parser is needed in the shell: Rust serializes the snapshot and
also writes a simple numeric mapping manifest and fingerprint file for PID 1.
Control failures are errors, not hash mismatches. For relay-status failure on an
otherwise compatible running container, warn that forwarding health is unknown and
preserve shell access. Missing/corrupt snapshot is an incompatible runtime error;
never substitute current config as a silent recovery.

A later start/attach/exec warns on degraded forwarding and proceeds using the frozen
snapshot if hooks succeeded. Existing hook failure handling remains unchanged;
report a direct Docker shell command for diagnosis when bootstrap itself failed.
Run-without-command listing also uses the frozen named-command catalog when running.
Keep startup and restart diagnostics bounded and payload-free; retain only the last
attempt's limited error tail per port, with total output caps enforced by the CLI.

## Immutable Launch Snapshot

Introduce an internal `RuntimeSnapshotV1` DTO, separate from `DevcontainerConfig`.
It contains protocol version, logical profile identity, launch token, image ID,
configuration fingerprint, effective container user/workdir, substitution context,
resolved default command, ordered resolved attach hooks, resolved project/Feature
named commands, forwarding plan, and relevant launch options for drift diagnostics.
Resolve configuration-defined substitutions once. Exclude host lifecycle commands,
registry-CA contents, Docker arguments, host mount specifications, and executable
host actions. The host never turns a returned snapshot into a launch/build plan.
Capture the full substitution context, including the configured user's HOME/USER
using the existing environment-probe mechanism on new creation only, so later explicit
commands do not require a new probe container merely to resolve those variables.

Create a unique launch directory under `.dcc/<profile>.rt/instances/` using exclusive
creation. Host outer instance directory is mode 0700; a `payload/` subdirectory is
0755 and is mounted read-only at the existing runtime-assets location. It contains
startup scripts, mapping manifest, fingerprint, and `snapshot.json` (0600, owned by
the invoking host user). Container root reads the snapshot through the fixed control
verb; the normal supervisor user only needs the manifest/scripts. Binding `payload/`
directly avoids exposing host-private ancestor directories inside the container.
Use platform-equivalent access restrictions where POSIX modes are unavailable.
Fail clearly if Docker's user mapping prevents the required root snapshot read;
never make the snapshot world-readable to work around it.

No snapshot or resolved secret goes into labels, argv, logs, or image layers. Only
nonsecret logical identity/launch token labels are added. Secrets already required
by lifecycle scripts retain their existing in-container visibility. Do not follow
pre-existing symlinks when writing assets; write everything before creating the
container. The directory remains immutable while any container references it.
Build-prep receives a separate instance directory and no runtime relay manifest.

Read snapshot output with a 16 MiB cap, validate schema, lengths, identity, absolute
workdir and numeric ports, and use structured Docker argv. Treat it as container-side
input, not host authority. Compare immutable image ID/launch token to Docker inspect.
Malformed data fails closed with stop/rebuild/recreate guidance. Host code must not
read paths or execute host hooks supplied by this data.

Clean known abandoned instance directories only after successful Docker inspection
proves no running or stopped container references them. On uncertain state, retain
them. No background garbage collector; prune on a later launch/build. Keep local
cleanup confined to dcc-created instance names under the trusted profile root.
Remote Docker retains the existing requirement that workspace/bind paths are usable
by its daemon; the snapshot does not add file synchronization or a new transport.

## Cheap Drift Detection And Frozen Reuse

Compute a versioned SHA-256 over length-delimited records, with deterministic ordering:
configuration inheritance paths and bytes, referenced local-environment names and
values (distinguish absent/empty), and directly named small local inputs already
available to planning: Dockerfile, applicable ignore file, Feature lockfile, local
Feature metadata/install entrypoint, and registry-CA files. Include a missing marker
for optional files and the effective profile/workspace substitution roots. Source
bytes deliberately allow warnings for comment-only changes and preserve Feature order.
Bound each auxiliary file read to 16 MiB; an over-limit/unreadable input makes the
comparison incomplete and produces a warning, rather than silently claiming a match.
Recompute the input list from current trusted host configuration, never snapshot paths.

Do not recursively hash build contexts, state directories, application source or
Feature trees, inspect mutable bind contents, or resolve/poll remote tags. Those
changes are outside inexpensive detection. No separate build fingerprint initially.
A source file may change while read; this is best-effort detection at invocation,
not a filesystem transaction or continuous watcher.
Also compare the current local image tag's ID with the snapshot's image ID using
read-only Docker inspection. A changed/missing tag warns; it never changes the
actual-image compatibility check or triggers a pull. Failure to inspect it produces
an incomplete-comparison warning and does not prevent frozen reuse.

For a known running instance, order work as follows:

1. Derive workspace/profile identity and discover runtime before loading/preparing
   current config. Resolve one actual container ID; reject ambiguous matches.
2. Inspect its actual image ID/version, then fetch and validate its launch snapshot.
   Do not use the current mutable image tag as compatibility authority.
3. Compute current fingerprint without cache/state creation, hook rewriting,
   lifecycle execution, environment probe containers, or remote Feature fetches.
   Compare through `verify-config`; warn once per invocation on drift or inability
   to compare, and say stop/rebuild where needed/recreate applies changes.
4. Wait for hooks if still bootstrapping, inspect relay health, and execute using
   frozen user/workdir/hooks/named commands. A changed name, removed script, port
   list, or environment does not partially update the running container.

Malformed/unreadable current config warns and uses the snapshot when identity is
known. Normal named profiles do not require parsing or existence of the config file.
If workspace/path-based identity cannot be recovered (for example removed parent
workspace or symlink), report that limitation and direct the user to the known Docker
container; do not guess and attach to another project. Dry-run keeps its current
zero-Docker behavior and explicitly says reuse/snapshot/drift checks were skipped.

Explicit command argv, terminal choice, debug, lifecycle skipping, and `--keep` are
invocation controls. They can differ per session. Explicit argv substitutions use
current localEnv and the stored container substitution context; configuration-defined
commands/hooks use their already resolved snapshot values and are never substituted
again. Changed creation-only CLI options (such as resource limits) warn and defer;
omission does not request resetting the running instance's limits. Existing durable
promotion remains explicit and must actually update PID 1's observed mode.

## New Creation, IPv6 Fallback, And Races

Only the no-running-instance branch loads/validates current config, checks image
compatibility, materializes mounts/state and immutable assets, and generates the
mapping plan. Resolve the image tag to an immutable image ID for the actual creation.
Use Docker create followed by start to retain ownership of the exact attempt. Attach
a unique nonsecret launch token and use exact IDs for subsequent control/cleanup.
Retain auto-removal for containers that successfully run and later exit.

Try IPv4 plus IPv6 first. If creation/start fails, retry once with IPv4-only only
when the attempt is proven never started (created state and zero StartedAt), or
creation failed with no container and no start was issued. Remove only this attempt's
exact ID after checking its token. Never remove or retry an existing reused container,
a name-conflict winner, a container that ran hooks, or an attempt with uncertain state.
If auto-removal/inspection failure loses that proof, return the error instead of
risking duplicate hooks. Reconcile a failed create by token/name before claiming it
left no container. The retry is allowed for any proven pre-start failure, avoiding
fragile Docker-error string matching; report both errors if IPv4 also fails.

After start succeeds, inspect actual PortBindings/NetworkSettings and require every
IPv4 mapping. Warn for omitted/unavailable IPv6 and report the active family set.
Unexpected wildcard or absent required mappings fail startup; no success based on
intent alone. If an allocated proxy port is unexpectedly published on a non-loopback
address, stop this owned new instance rather than retaining the unsafe publication
for diagnosis. Then, for valid mappings, perform strict `wait-ready`. Never retry
creation because a hook or in-container relay failed. Remote or indeterminate Docker endpoint location is
reported as "Docker daemon host's localhost", never promised as CLI-machine localhost.

For concurrent launches, a deterministic container name is the Docker arbitration
point. A losing client rediscovers the winner and takes the frozen reuse path; it
never removes the winner or changes its assets. Distinct launch directories avoid
clobbering even before arbitration. Do not add distributed locks or reconciliation.
The preflight/build race between independent clients remains an explicit limitation.

## Stop-Before-Build Rule

In `src/build.rs::build`, after the zero-Docker dry-run return and before either
build/refresh branch, query the same logical profile's runtime. A running runtime
refuses build, refresh-only, and reseeding with instructions to stop first. Lookup
failure also refuses; build-prep containers do not count as runtime. Do not stop
anything automatically or enqueue preparation. This avoids ordinary shared-state and
hook mutation while running, without claiming atomicity against concurrent clients.

## Implementation Slices And Verification

| Slice | Main code | Required evidence before integration |
| --- | --- | --- |
| Planning/snapshot | `config/{mod,merge,resolve}.rs`, new `runtime_snapshot.rs`, `run.rs` | Range validation/exhaustion; deterministic fingerprint; inheritance/order/env absent-empty; frozen commands; bounded schema/secret handling. |
| Reuse and build gate | `exec.rs`, `main.rs`, `build.rs`, `supervisor.rs` assets | Fake-Docker proves reuse never builds/probes/rewrites/hydrates; malformed config stays accessible; actual image gate; build refusal has no mutations; dry-run calls no Docker. |
| Relay service | `forward.rs`, `supervisor.rs`, `features/{mod,context}.rs` | Package/argv contract; readiness inode ownership; fork isolation; stop/crash cleanup; bounded retries; hook-versus-relay health; one-shot and durable lifecycle. |
| Docker integration | `docker.rs`, `exec.rs`, `stop.rs` | Exact mappings, proven-never-started fallback, hook-at-most-once, ambiguous attempt failure, concurrent winner reuse, remote diagnostics and legacy stop. |
| Migration/docs | README and public config/runtime docs, compatibility tests | Explicit stop/build/recreate instructions, package/range/network/timeout behavior, minor version gate. Remove host relay path only when the new path is complete. |

No production change is complete before the appropriate format/check/Clippy/test/build
checks from project standards and the following integration matrix pass in a capable
environment. These are future checks, not results of this design task:

- Two simultaneous attaches and a background `start` share one mapping set; sessions
  ending do not release it. Missing application starts later without relay restart.
- Many concurrent independent streams, both directions of half-close, prompt complete
  response, no-data closing wait near two seconds, continued traffic beyond two seconds,
  and open idle connections surviving beyond two seconds. Avoid exact timing assertions.
- Foreign listener must not pass readiness; port collision and package/procfs failures
  are clear; listener crashes clean descendants, retry only three times and leave
  diagnostic access; graceful/forced stop and one-shot exit remove every relay process.
- Drift in hooks/scripts/env/ports/build/local Feature inputs warns without mutation;
  corrupt current config, snapshot corruption/oversize, and secret-log redaction.
- Linux Engine and Docker Desktop localhost v4/v6, IPv6-unavailable fallback, host-port
  collision, app IPv4 loopback, allowed network-peer access, and non-root images on
  Debian/Ubuntu, Alpine, Fedora-family. Include user mapping and remote bind-path limits.

Rollback is explicit: stop the new container, select a matching older CLI/image pair,
and recreate. No migration of cache/state data and no automatic image/container
replacement. Implementation tests must verify packages rather than relying on manuals.

## Design Evidence And Review

The old foreground path in `exec.rs` unconditionally starts `forward_ports` after
reuse. `RuntimePlan::prepare` mutates assets before that lookup; `run.rs` independently
loads mutable scripts; build preparation hydrates shared state. These are the source
boundaries behind the selected changes. PID 1 is a POSIX shell, so the design adds
small fixed service/control scripts, not a general JSON/service-management subsystem.

Sources checked 2026-09-28: [socat manual](https://manpages.debian.org/trixie/socat/socat.1.en.html)
and [original socat 1.8.0.3 source](https://deb.debian.org/debian/pool/main/s/socat/socat_1.8.0.3.orig.tar.bz2)
(`socat.c` closing-loop reset, lines 1065–1069); [Docker publishing](https://docs.docker.com/engine/network/port-publishing/),
[create](https://docs.docker.com/reference/cli/docker/container/create/),
[start](https://docs.docker.com/reference/cli/docker/container/start/);
[proc socket descriptors](https://man7.org/linux/man-pages/man5/proc_pid_fd.5.html),
[Linux TCP proc format](https://docs.kernel.org/networking/proc_net_tcp.html), and
[setsid](https://man7.org/linux/man-pages/man1/setsid.1.html).

[Quality/readiness record](../quality/0096-container-relay-design.md) and
[threat review](../threat-models/0096-container-relay-design.md) own review evidence.
This design exceeds the usual task-note length to keep the intertwined snapshot,
relay, and creation protocol in one reviewable contract; execution history is omitted.
