# T-0103: Codex Profile Evaluation

2026-11-16. User authorized evaluating a Codex-equipped dcc profile with Docker
and sudo. Scope: build, durable startup, exec, interactive attach, Codex UI startup,
and persistence across stop/start. No product changes or credential transfer.

## Environment and fixture

- Amazon Linux 2023 ARM64; Docker Engine/CLI 29.9.3, Buildx 0.39.0.
- dcc 0.2.1 debug binary from current product source (repository HEAD b5ca3be).
- Related features checkout HEAD 068a82b; tested the published OCI feature rather
  than replacing its source with the local checkout.
- Existing Docker group activated through `sg docker -c`; no socket policy changes.
- Workspace `/tmp/dcc-codex-evaluation`, profile `.devcontainer/devcontainer.json`:

```json
{"name":"Codex evaluation","image":"debian:bookworm","features":{"ghcr.io/dc-powertools/features/codex:latest":{}}}
```

## Observed results

All commands ran from the fixture workspace using
`/home/ec2-user/workspace/dcc/target/debug/dcc` under the Docker group.

- `dcc build`: passed. Published feature downloaded the official installer and
  installed Codex CLI 0.169.0 for aarch64-unknown-linux-musl. Image built and
  declared state seeded successfully. Debug-build state hashing used substantial
  CPU and approximately 500 MB RSS for roughly 471 MiB of installed Codex state.
- `dcc start`: passed. Durable runtime became available; profile list JSON reported
  `running: true`.
- `dcc exec sh -c ...`: passed. uid 1000(dev), gid 993(dev), `/workspace` working
  directory, HOME `/home/dev`, CODEX_HOME `/home/dev/.codex`, writable state,
  `/usr/local/bin/codex`, `codex-cli 0.169.0`, and `/usr/bin/ps`.
- `dcc attach` with a real PTY: passed. Interactive shell ran `codex --version`,
  printed ATTACH_OK, and exited successfully; durable container remained running.
- `dcc attach codex` with a real PTY: passed. Codex displayed its interactive
  welcome and authentication-choice screen. Exited with Ctrl-C without logging in.
- `dcc stop`, `dcc start`, then exec: passed. Marker written in CODEX_HOME survived
  recreation and Codex still reported its version; RESTART_PERSISTENCE_OK observed.
- Final `dcc stop`: passed. `docker ps -a` showed no remaining containers.

During initial observation an early start overlapped the build's state hashing and
was terminated before container creation. Final startup checks ran only after build
completed. Its temporary missing-ledger warning is not the final result.

## Findings and limits

Startup emits a misleading seed warning even immediately after a successful build:
ledger and label both contain `dcc-d90b551bf2b1--devcontainer`, but the comparison
uses image digest `sha256:c6eabaf322d9c1310aa8ea1c9afc476afcc54b32368ee42306f201d02faaa43f`.
Source `src/exec.rs` compares `image_manifest.build_id != image` in
`runtime_seed_guard`. Warning did not prevent startup, attachment, or persistence.
No fix was included in this evaluation.

Successful connection here means dcc attachment and Codex UI startup. Authenticated
OpenAI service access, model requests, browser callbacks, other distributions,
x86_64, and the full multi-feature dcc development profile were not tested.
No credentials were supplied or copied. This does not complete T-0097's platform
qualification matrix. Fixture, image, state, build log, and image-label snapshot
remain under `/tmp/dcc-codex-evaluation` for inspection; runtime containers removed.

Focused review: observed commands support the scoped success verdict; no product
files changed. Documentation diff and whitespace checks passed before local commit.
