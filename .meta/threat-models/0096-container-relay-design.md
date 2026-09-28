# T-0096: Relay And Launch Snapshot Threat Review

Design-only review, 2026-09-28. Builds on the
[existing runtime boundary](0004-dcc-runtime.md) and
[design](../tasks/0096-container-relay-design.md). No mitigation is implemented yet.

Assets: host-local application access, configuration/environment secrets, shared
workspace/state, and reliable container/session lifetimes. Trust boundaries: Docker
host versus container network; mutable current config versus frozen launch state;
container-returned snapshot versus host execution; package repositories versus images.

| Threat | Design mitigation / future verification |
| --- | --- |
| Wildcard publication or peer access mistaken for host-only isolation | Explicit loopback mapping plus actual binding inspection; reject publish-all; qualify normal NAT bridge on Engine >=28. Peer-container exposure is explicitly accepted in decision 0008. Test actual reachability. |
| Snapshot reveals environment secrets through labels/logs/files | Private host parent and 0600 JSON, read-only container bind, root-only fixed-path control read, bounded output, no snapshot debug dump. Test host and container ownership with UID mappings. |
| Container-provided snapshot triggers host hooks, file reads, mounts, or cleanup | Limited DTO contains no host actions. Derive fingerprint inputs from trusted host config. Check identity/image/token; structured argv. Cleanup uses host-generated instance paths and Docker reference checks. Fuzz malformed/oversized snapshots and symlinked assets. |
| Current config changes partially take effect before drift warning | Discover/reuse before all mutating preparation; snapshot supplies hooks/scripts/user/workdir; explicit stop-before-build preflight. Fake-Docker asserts forbidden calls never occur. |
| IPv6 fallback deletes another container or repeats startup hooks | Exact attempt ID/token and proven never-started predicate; uncertain state fails without retry. Concurrent name winner is reused. Negative tests include timeout, auto-removal, nonzero StartedAt and name conflicts. |
| Relay restart leaves workers or signals user-command groups | Dedicated service groups; retain/validate process identity; cleanup before retry; bounded restart count; no externally supplied PIDs. Test listener-only crash, descendant cleanup, stop/restart races. |
| Readiness mistakes an unrelated listener for the relay | Match socket inode and port to tracked listener PID; no application probes. Test foreign listener, procfs restrictions, PID exit/reuse and readiness timeout. |
| Unbounded diagnostics or unexpected command interpretation | Fixed numeric manifest, quoted fixed argv, no user-supplied socat options, output caps, no traffic logging. Test control characters and log-volume limits. |
| Dependency substitution or unsupported package capabilities | Use existing distro package channels and explicit capability qualification. No new remote binary download, privilege, compiler pipeline or CI permissions. |

Residual risks: trusted container root can subvert its own lifecycle, as already
accepted. Container peers can use the proxy; socat fork connections consume resources
under existing container limits. A closing wait may truncate sufficiently stalled
responses, explicitly accepted with `-t 2`. Simple build preflight is not distributed
coordination; simultaneous independent clients can race. Source hashing is bounded
and best effort. Daemon/network configuration remains part of the security boundary.

Agent review used source and official documentation as evidence, not instructions.
No downloaded source was executed, no container was launched, and no release or
permission changes were made. Implementation must pass the design's security and
platform matrix before these planned controls are described as verified.
