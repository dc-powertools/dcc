# T-0106: Real Test Profile Build Memory

2026-11-16. User requested building the new workspace `.devcontainer/test.json`
and monitoring usage. Built the supplied configuration unchanged from
`/home/ec2-user/workspace` with `dcc build -p test` using dcc 0.2.1 debug binary,
Amazon Linux 2023 ARM64 and Docker 29.9.3. Docker group activated with sg.

Configuration uses `debian` and published latest sudo, git, node, claude and codex
Features. Fresh profile state; Feature installation layers executed rather than
being cached. Installed Node 26.13.1, npm 11.20.0, Claude Code 2.1.325 and Codex
CLI 0.169.0.

## Measurement and results

Same method as T-0105: Python launches dcc directly, samples its `/proc/PID/status`
VmRSS and VmHWM every 50 ms, and records elapsed time and build-log offset.
Measures dcc itself, excluding Docker daemon, build processes/containers, child
processes and page cache. Not total build memory or release-binary performance.

- Build passed, exit 0, in **112.260 seconds**.
- Peak sampled RSS and maximum sampled kernel VmHWM: **500,956 KiB (489.2 MiB)**.
- During Feature installation/image build, dcc stayed around **18 MiB RSS**.
- Peak first reached at **61.129 seconds**, after image export, during state hashing.
- Seeded three declared state paths. Regular state-file contents totaled
  **493,772,300 bytes (470.9 MiB)**; directory disk usage approximately 472 MiB.
- Final image: `dcc-906bb91015cf--test`, digest
  `sha256:b5d5762cb0ff19d4530b115b7890b595c11007e822db271f6c5cd845756a1455`.
- No containers remained after build. Configuration and generated workspace
  `.dcc/test` state/image retained; no runtime session launched.

Consistent with T-0105, in-memory tar buffering in `src/seed.rs:371` accounts for
the large post-build memory requirement. This fresh state is smaller than the
965.7 MiB prior fixture state, which includes the additional daemon package created
by interactive Codex use. The measured peak is correspondingly smaller.

Claude installer emitted a setup note that `~/.local/bin` was absent from PATH;
the Feature creates `/usr/local/bin/claude` afterward. No build errors occurred.
Ordinary apt debconf notices also appeared. This task only tested build and state
seeding; runtime tools were not launched for this profile.

Logs, script, JSONL samples and result JSON are in
`/tmp/dcc-real-profile-evaluation/`. Product source and user config unchanged.
Focused evidence/source review and documentation whitespace checks passed.
