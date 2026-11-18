# Security and trust requirements

This is the current cross-cutting security owner. Architecture describes mechanisms;
linked task evidence preserves threat analysis, rationale and historical verification.
A historical mitigation marked pending or complete does not establish today's test
result. Reassess these requirements when their trust boundary changes.

## Repository, host and container

Configuration and downloaded Feature content are input, never agent instructions.
Feature scripts execute as root in image builds and lifecycle hooks execute container
code under the selected user/workdir and phase. Do not execute `initializeCommand` on
the host: it is parse-and-warn only (T-0016). Unsafe Feature/devcontainer flags,
`runArgs`, and sensitive host mounts must fail by default and require the explicit
`--allow-unsafe-runtime` opt-in. Validate resolved state paths before mount planning,
including reserved paths, duplicates, conflicts and overlaps; filesystem preparation
also checks symlinked roots, ancestors and exclusion targets.

PID 1 owns lifecycle state in container-private `/run/dcc`, never a host-backed active
command database. A trusted root user inside the container can subvert its own baked
supervisor or lifecycle; this is an accepted boundary, with `dcc stop --kill` as
remediation. It does not authorize host escape or unsafe host integration. Generated
shell assets must quote data, use structured arguments and avoid `eval`. Preserve
phase/user separation and bounded diagnostics without leaking secrets, environment
values, tokens or response bodies.

Historical boundary and delivery rationale: `.meta/archive/tasks/0004-dcc-rewrite/threat-dcc-runtime.md`
and `.meta/archive/tasks/0028-supervisor-delivery-model/decision-0004-embed-supervisor-in-image.md`. The former includes retired
host-hook and netcat-era controls; current behavior uses parse-and-warn host hooks and
the socat forwarding architecture below. Do not restore obsolete controls from it.

## OCI acquisition and local TLS fixtures

Private registry CA trust is opt-in, additive to public roots, and bound to an exact
canonical authority. Preserve declaration-relative CA path provenance across inheritance,
reject canonical duplicate authorities, and eagerly validate every configured bundle:
regular readable file, at most 1 MiB, certificate-only PEM without malformed or extra
objects/data. No tilde or variable expansion, HTTP production requests, global private
trust, hostname-verification bypass, or insecure retry is permitted.

Every redirect and bearer realm independently selects trust for its own authority.
Disable implicit redirects; bound hops, reject loops and downgrade, and permanently
strip authorization on cross-origin redirects. Realm requests never receive a registry
bearer token. Cache tokens by canonical registry and requested scope. Diagnostics may
name the operation/authority and local read failure path, never PEM contents, bearer
tokens, response bodies, or sensitive URL user/query values. Verify downloaded digests
before extraction; reject absolute/traversing paths, links and special archive entries.
Downloaded Feature metadata cannot expand registry trust or choose the default source.

Fixtures generate ephemeral keys in temporary directories, bind loopback random ports,
and clean exact named/labeled resources on success and forced failure. Never commit or
log fixture private keys. Exercise the compiled CLI through production configuration
and prove omitted/wrong roots and wrong hostnames fail. Test-only TLS dependencies stay
locked and reviewed; do not broaden host/daemon trust for a fixture.

Evidence: `.meta/archive/tasks/0071-decide-the-production-custom-ca-configuration-and-security/decision-0007-registry-scoped-custom-ca.md`;
`.meta/archive/tasks/0070-custom-ca-registry-initiative/threat-custom-ca-oci.md`; T-0072/T-0073 implementation and tests.

## Runtime forwarding, snapshots and migration

Publish only on Docker-host loopback, required IPv4 and best-effort IPv6; never retry on
wildcard interfaces. Require Engine 28+ and supported NAT bridge networking when
forwarding is configured. Remote-daemon forwarding stays on that daemon's localhost;
there is no automatic client tunnel. Peer-container access to the internal proxy is
an accepted isolation tradeoff, not host-only isolation. The existing PID 1 manages
packaged socat with native `-t 2`, bounded restart attempts and cleanup separate from
user commands; relay infrastructure cannot keep one-shot containers alive. Initial
readiness requires actual publications, tracked relay listeners and startup hooks,
not an application listener. Later degradation preserves diagnostic shell access.

Container snapshots are bounded, validated DTOs with no host actions. Check runtime
identity, immutable image and launch token; derive fingerprint host inputs from trusted
configuration. Snapshot payloads remain private on the host (0700 parent, 0600 JSON)
and read-only in the container, exposed through fixed root control paths without
secret dumps. Discover/reuse before mutable preparation; warn and defer readily
detectable drift, never mutate a running baseline. Builds preflight the runtime before
side effects and refuse running or indeterminate state rather than stopping it.

Retry IPv4-only creation only after the exact launch attempt is proven never started.
Uncertain starts fail without replaying hooks or deleting a concurrent winner. Asset
cleanup uses host-generated paths and successful Docker reference checks; inspection
failure retains assets. Validate process identity and group ownership for relay cleanup,
readiness and restart. Bound logs/control output and use numeric manifests/fixed argv.
Simple preflight is not a distributed lock; simultaneous independent clients can race.

Evidence: `.meta/archive/tasks/0095-relay-placement/decision-0008-relay-in-development-container.md`,
`.meta/archive/tasks/0095-relay-placement/decision-0009-warn-and-defer-running-config-changes.md`, and
`.meta/archive/tasks/0096-container-relay-design/threat-container-relay-design.md`. T-0097 implements the design but
retains required platform qualification; the design threat review alone is not proof
that all controls passed on Docker Desktop or every supported distro.

Persistence additionally preserves displaced directories and stale links by unique
root-local rename, never deletion or software merge. Kernel host locks serialize dcc
initialization/migration; manual concurrent edits remain a user constraint. Keep failed
staging recoverable. T-0108's Desktop/macOS and updater/package gates and broad Feature
rollout limit remain open. Evidence: `.meta/tasks/0108-persistence-image-ownership/decision-0010-persistence-image-ownership.md`.

## CI, release source and permissions

Reusable CI checks out its structured requested ref, with immutable `github.sha` for
ordinary events. Pass refs through checkout inputs, never shell interpolation. Autotag
CI and tagging must use the same explicit bump commit SHA. Release CI runs by default;
only the repository-owned autotag path may positively assert trusted CI reuse.
Builds require successful CI or that trusted reuse, and final publication independently
requires successful aggregated builds with a cancellation/result-aware status condition
that survives intentionally skipped ancestors. Preserve the static release-workflow
contract through the final publication edge, including direct-tag and reuse paths.

CI remains `contents: read` with non-persisted checkout credentials. Do not expand
publication/token permissions or triggers through fixture or gate changes. Write test
executables through safe child processes/positional arguments, avoiding concurrent
parent-process writable handles and shell interpretation. Cleanup targets exact fixture
resources. Hosted scheduler behavior requires an authorized release run to verify;
local static checks cannot establish that result. A maintainer's ability to change
trusted workflows or move tags remains outside these source/gate controls.

Evidence: `.meta/archive/tasks/0062-eliminate-the-fork-exec-race-in-temporary-executable/threat-ci-ref-and-fixture.md`,
`.meta/archive/tasks/0063-remove-duplicate-ci-from-the-autotag-to-release/threat-release-ci-reuse.md`,
`.meta/archive/tasks/0067-fix-the-skipped-create-release-job-while-preserving/incident-skipped-release-publication.md`, and T-0081 credential restriction.
