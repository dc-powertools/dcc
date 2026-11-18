# T-0111: Cold startup status 137 reproduction

User request: reproduce locally; no product fix requested or applied.
Environment: Linux aarch64, Docker Engine 29.9.3, checkout a53401d; freshly built debug binary. Docker invoked through sudo in isolated /tmp fixtures; container user dev (UID 1000).

## Exact reproduction

Fixture: `/tmp/dcc-repro-0111-r39po4qr`; image `dcc-a9b17b6d60c1--devcontainer`. Configuration:

```json
{"image":"debian:bookworm-slim","postStartCommand":"sleep 1","forwardPorts":[3000],"customizations":{"dcc":{"commands":{"probe":"/bin/true"}}}}
```

After `dcc build`, `dcc --debug run probe` reproduced the exact `container startup (retained for diagnosis where possible)` status 137 twice, in 14.548 and 13.590 seconds. No Codex, credentials, or substantial workload required. Events in `/tmp/dcc-repro-0111-r39po4qr/events.jsonl` show readiness exec exiting 137 followed by container die exitCode=0 and destroy; no OOM event in captured sequence.

While blocked, `/run/dcc/bootstrap-status` contained 0, but the wait-ready process was blocked opening a FIFO (`wait_for_partner`). Its FIFO in `/run/dcc/waiters` was root:root 0644. The dev supervisor could not write it. `docker::control` explicitly invokes docker exec -u 0; ctl_script uses mkfifo without changing ownership or permissions. supervisor signal_waiters attempts read/write opening and silently ignores failure. A root-created waiter can therefore never receive the non-root supervisor notification. Timing explains success when the status file exists before the waiter checks it.

One-shot supervisor reaps an unregistered command after bootstrap +10 seconds, killing the blocked readiness exec; relay shutdown can add time. This explains 137 without container OOM.

## Durable comparison and causal check

On this fixture `dcc start` also blocked (durable mode avoids the reaper). In the temporary test container only, changed waiter permissions to 0666 and sent the normal FIFO notification as dev. The pending start immediately completed successfully (total 29.061 seconds including manual diagnosis); `dcc run probe` then succeeded in 0.386 seconds. After stop, cold run reproduced the original startup error again. No persistent source/configuration change was made for the permission experiment. This indicates the user's start workaround is timing-dependent, not guaranteed.

An immediate --keep retry during the dying-container window returned `cannot read running container snapshot`; it did not establish a reliable workaround.

## Other observations and limits

- Minimal no-hook Debian fixture passed four cold execs, durable start/exec, stop, and cold exec.
- 30 cold named runs with a 0.05-second hook passed.
- Existing local Codex pilot image: three cold codex --version runs yielded 0, 137, 0. The 137 was a separate post-readiness fast-command teardown race: readiness and snapshot execs succeeded, then command exec died 137 while container exited 0.
- Noninteractive codex:yolo reached Codex and rejected non-TTY stdin, before and after durable start and stop. Interactive authenticated Codex behavior was not tested.
- No reproduction containers remain running. Temporary fixture files/images are retained for follow-up. Existing unrelated containers and project state were untouched.

Result: exact reported startup error reproduced and FIFO ownership/permission cause verified. Product remediation remains separate work.
