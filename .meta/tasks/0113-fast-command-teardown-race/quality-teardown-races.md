# T-0113: Deterministic teardown race confirmation

Date: 2026-11-17. Investigation authorized by user; product implementation pending.
Environment: Linux aarch64, Docker Engine 29.9.3; working checkout 3f07f42.
Image: dcc-5bc6cc1e83e0--devcontainer:latest (sha256:d86c3a8fe351cc6c2d91a6b3cec7e9db4c795db0fecf8b8306e90da590264592).

## Method

Extract baked dcc-supervisor and dcc-exec from the existing Debian test image.
Make temporary copies, introducing a checkpoint in just one script per case.
Bind-mount that script over its baked path in an isolated container. Launch the
supervisor as dev in oneshot mode with --expect-command and /run/dcc on tmpfs
(mode 1777), without startup hooks or relays. Other runtime scripts stay unchanged.
Checkpoints lengthen scheduling windows; they do not change the active-count
result, registration ordering, shutdown condition, or command exit status.

### Stale-count case

Immediately after `n=$(active_count)` in the supervisor loop, on its first pass:

```sh
if [ ! -f "$STATE/repro-count-paused" ]; then
    : > "$STATE/repro-count-paused"
    while [ ! -f "$STATE/repro-release-count" ]; do sleep 0.02; done
fi
```

Wait until that checkpoint is reached, then Docker exec the unmodified wrapper
with `sh -c 'echo user-command-started; : > /run/dcc/repro-command-started; sleep 60'`.
Observe the started marker, an active record, and the arrived marker before
creating /run/dcc/repro-release-count. The supervisor resumes with n=0 but sees
ARRIVED, so it shuts down despite the registered command still running.

### Wrapper-exit case

Leave the supervisor unchanged. In dcc-exec, replace the normal EXIT trap with:

```sh
trap 'rm -f "$record"; : > "$STATE/repro-wrapper-exiting"; while [ ! -f "$STATE/repro-release-exit" ]; do sleep 0.02; done; exit "$status"' EXIT
```

Wait for bootstrap status, then Docker exec this wrapper with
`sh -c 'echo user-command-finished; exit 0'`. The user command succeeds, the EXIT
trap removes the record, and the wrapper remains alive before its final exit.
The supervisor sees the empty active set and exits, killing the wrapper.

## Observed results

| Case | Exec output | Docker exec status | Container exit | OOMKilled |
| --- | --- | --- | --- | --- |
| Stale count | user-command-started | 137 | 0 | false |
| Wrapper exiting | user-command-finished | 137 | 0 | false |

Both cases completed within the 10-second command wait bound. Inspect state and
container exit status were collected before each container was explicitly removed.
The stale-count case observed an actual active record before supervisor release.

These are deterministic demonstrations of two distinct unsafe interleavings.
They do not establish which interleaving caused the original uninstrumented Codex
failure. No repository source changes or shipped regressions were made here; a
future fix must add appropriate regression coverage. Test containers were removed.

Temporary reproduction script: /tmp/dcc-0113-races.py. Original and instrumented
scripts and results.json: /tmp/dcc-0113-races-5e1__ayh. This document preserves the
checkpoints and results independently of those temporary paths.

The user subsequently favored the current container-owned approach after comparing
CLI-crash consequences: explicit acknowledgment introduces unreleased reservations
even when command wrappers exit normally. It is not the selected design direction.
Both confirmed races remain unresolved, and T-0113 stays open for further research.
The crash comparison, conclusion, and remaining lifecycle questions are owned by the
[T-0113 brief](fast-command-teardown-race.md).
