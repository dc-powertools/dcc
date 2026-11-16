# T-0105: dcc Build Memory Measurement

2026-11-16, Amazon Linux 2023 ARM64, Docker 29.9.3, current dcc 0.2.1
**debug** binary. User requested measuring dcc memory during build.

## Method

Launched dcc directly as a Python subprocess under the existing Docker group and
sampled its own `/proc/PID/status` VmRSS and VmHWM every 50 ms, recording monotonic
elapsed time and build-log byte count. Excludes Python, Docker CLI children,
Docker daemon/BuildKit, containers, and filesystem cache. This is process RSS,
not total host memory consumed by the build. Sampling can miss brief RSS spikes;
VmHWM provides the kernel peak as of each sample. One run per profile; no code edits.

Measurement scripts, JSONL samples, build logs and result JSON remain under
`/tmp/dcc-codex-evaluation` and `/tmp/dcc-memory-baseline`.

## Results

| Profile | Peak sampled dcc RSS | Elapsed | Result |
| --- | --- | --- | --- |
| Debian bookworm + published Codex Feature, existing state | 1,012,076 KiB (988.4 MiB) | 106.635 s | Exit 0 |
| Debian bookworm, no Features or declared state | 15,796 KiB (15.4 MiB) | 1.717 s | Exit 0 |

Codex build reused cached Docker layers and completed image export before the
RSS spike. Peak sampled VmHWM matched peak RSS. Existing Codex state contained
1,012,621,843 bytes (965.7 MiB) across 2,623 regular non-symlink files, including
standalone and app-server-daemon installations created by earlier interactive tests.
The process reached 988.4 MiB at 2.578 seconds and retained that RSS throughout
state hashing. Before export finished, highest sampled RSS was 18,132 KiB.

## Explanation and limits

`src/build.rs` hashes existing declared state before deciding whether to hydrate it.
`src/seed.rs:371` creates `tar::Builder::new(Vec::new())`; lines 404–405 finalize
that entire archive in memory and then pass it to SHA-256. Despite the function's
streaming comment, it buffers the archive. Memory therefore scales with the size
of the state being hashed, consistent with the measured approximately 966 MiB state
and approximately 988 MiB process RSS. File-state hashing also reads whole files.
Streaming archive output directly into a hash writer is a potential improvement;
not implemented as part of measurement.

The build correctly warned that Codex state had changed and preserved it; this is
expected after prior Codex launches, distinct from the earlier tag/digest warning.
No state reset, credentials copied, or existing user data overwritten. No runtime
containers remain. Baseline fixture and built images retained for inspection.

These figures describe cached local builds using a debug binary, not release-binary
performance, a cold build, total Docker memory, or an upper bound for other profiles.
Initial fresh-install state in T-0103 was smaller; current state includes the daemon
package. No release-speed claim is inferred from the 106.6-second debug run.
Focused source/evidence review and documentation whitespace checks passed.
