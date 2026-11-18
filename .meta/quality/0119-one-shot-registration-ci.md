# T-0119: One-shot CI status 137 before the command marker

Date: 2026-11-17. User requested diagnosis; no product fix authorized or applied.
Checkout: 1197c2c; actual CLI reports dcc 0.3.1. Linux aarch64, Docker 29.9.3.

## Meaning of the supplied failure

one_shot_container_drains_after_command_without_grace configures containerUser=root,
so the non-root workspace write failure from T-0116 does not explain this test.
Its marker is written by the user shell, not by the wrapper at actual registration:

```sh
printf ready > /workspace/one-shot-ready; while [ ! -f /workspace/one-shot-release ]; do sleep 0.05; done
```

Actual registration happens earlier in dcc-exec: create the active record, set
ARRIVED, install traps, wait-hooks, then invoke the user command. ChildGuard's
`before one-shot command registration` wording therefore names a later milestone
than the actual registration. A process exit 137 before that file is compatible
with killing an already-registered wrapper before it reaches the user command.
The test also discards stderr, limiting the diagnostic content of the CI message.

## Exact CLI-level failure mechanism reproduced

Used the actual dcc 0.3.1 binary, the test's root-user Debian configuration, and
its exact shell command in an isolated temporary project. Built normally.
Extracted the freshly built supervisor/wrapper scripts and made temporary copies
with two scheduling checkpoints. A test-only Docker CLI shim injected read-only
mounts for those copies into runtime create; it otherwise delegated Docker calls
unchanged. No repository code changed.

1. Pause supervisor immediately after its first n=$(active_count), which sampled
   an empty active set before the command wrapper was launched.
2. Allow normal CLI wait-ready and snapshot controls to succeed, then launch the
   foreground wrapper. Pause it immediately after setting the real ARRIVED marker,
   before executing the user command.
3. Inspect state: an actual active record existed, ARRIVED existed, and the test's
   one-shot-ready marker did not. This establishes completed registration but no
   user-command marker.
4. Release only the supervisor checkpoint. It combines cached n=0 with newly
   observed ARRIVED, considers the one-shot container drained, and exits normally.
5. The real dcc invocation returns 137 with no stdout/stderr; the ready file never
   appears. Docker events show successful readiness/snapshot execs, foreground
   exec_die exitCode=137, then container die exitCode=0 and destroy. No oom event
   appeared in the captured sequence.

The checkpoint code lengthens existing scheduling windows; it does not change the
real active count, registration ordering, ARRIVED value, or shutdown condition.
The wrapper pause is before cleanup/command completion, so this case specifically
confirms the stale-count/registration race, not T-0113's separate exit-trap race.

An ordinary unmodified run of the exact ignored smoke test passed in 2.46 seconds.
That is consistent with an intermittent scheduling race, not proof of its absence.
Git blame dates the active-count/ARRIVED logic to August 2026, before the recent
FIFO fixture corrections and patch bump.

The observed outcome would trigger the supplied ChildGuard panic: the real CLI
has exited 137 before the marker. It does not require a user command that exits
quickly, host UID mismatch, a readiness FIFO failure, or a slow CI marker timeout.

## Attribution and follow-up

This is a direct reproduction of the existing T-0113 stale-count mechanism at the
same marker boundary as the reported CI test. The supplied panic alone, without
that CI run's Docker events/kernel logs, cannot rule out every other source of
137 in that particular run. The reproduced race is a sufficient explanation and
is already known to be present in the current supervisor. A passing ordinary
smoke run does not exclude this scheduling-dependent failure.

Repair remains in [T-0113](../tasks/0113-fast-command-teardown-race.md): synchronize
registration/admission and the transition to draining so shutdown cannot use a
count rendered stale by newly admitted work. Preserve the selected container-owned
cleanup direction. Neither a longer test timeout nor the T-0117 synchronization
mount fixes this supervisor race. The separate wrapper-exit race also remains open.

Consider improving this test's marker wording to `user-command startup` and
retaining useful failure diagnostics, but do not mask 137 or replace correctness
with a fixed grace delay.

## Artifacts and limits

- /tmp/dcc-0119-reproduce.py
- /tmp/dcc-0119-xv4693a8/results.json
- /tmp/dcc-0119-xv4693a8/events.jsonl
- /tmp/dcc-0119-xv4693a8/supervisor and wrapper (checkpoint copies)
- /tmp/dcc-0119-unmodified-test.log (ordinary single-run comparison)

The isolated test container and image tags were removed. Temporary evidence was
retained. No production/test source changed; T-0113 remains open. This document
preserves the method and results independently of temporary artifacts.
