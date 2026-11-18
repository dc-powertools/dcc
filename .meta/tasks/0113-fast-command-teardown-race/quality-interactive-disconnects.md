# T-0113: Interactive disconnection observations

Date: 2026-11-17. User authorized local disconnect experiments; no product implementation.
Environment: Linux aarch64, Docker Engine 29.9.3, dcc checkout/binary 3f07f42;
tmux 3.3a, GNU Screen 4.09.00, Codex CLI 0.169.0.

## Setup and method

Built isolated temporary dcc projects from a local Codex pilot image, adding tmux
and screen in a disposable tools image. Tools image ID:
sha256:5d247a699940dc4d71c1f0db17274d8b0292dc13c865dd7bfb4e9f81d9404c66.
Configured containerUser=dev, updateRemoteUserUID=false. No credentials were copied
and no coding request was submitted to Codex.

Durable cases first used dcc start. One-shot cases cold-launched dcc exec without
--keep. These terms describe runtime lifetime, not different persistence backends.
Every interactive dcc invocation used a real local PTY with a controlling terminal,
TERM=xterm-256color, and a 100x30 window. The harness answered cursor-position
queries so Codex could render its TUI.

The input-waiting probe was a shell script that recorded START, its PID, received
input, EOF, and trapped HUP/INT/TERM. It read lines until EOF or a quit command.
The mux cases ran the same probe inside tmux/screen. A separate finite probe slept
three seconds and recorded DONE before exiting. Before and after each disconnect,
collected container state, active records, process listings, mux session listings,
probe events and local CLI status. Raw terminal output was retained.

Actions: Ctrl-D; Ctrl-C; Docker Ctrl-P Ctrl-Q detach; PTY-master closure (simulated
terminal loss); SIGKILL to dcc alone while leaving the PTY open; SIGKILL to the
specific host Docker exec client; tmux Ctrl-B d; screen Ctrl-A d.

## Observed consequences

| Action/scenario | Durable container | One-shot container |
| --- | --- | --- |
| Ctrl-D in input-waiting probe | Probe exited 0, record cleared, container remained | Probe exited 0, container removed |
| Ctrl-C in probe | Probe exited 130, record cleared, container remained | Probe exited 130, container removed |
| Docker detach from waiting probe | Job and active record remained; new exec worked | Job and active record remained; new exec worked |
| Terminal loss while probe waits | Job and record remained, no probe HUP/EOF recorded | Same; container stayed running |
| Kill dcc alone | Job and record remained | Same; container stayed running |
| Kill foreground Docker client | Job and record remained | Same; container stayed running |
| tmux/screen own detach | Client/wrapper exited 0 and record cleared; server/job survived and reattachment worked | Client/wrapper exited 0; supervisor removed container, killing mux server/job |
| Docker detach or terminal loss while running tmux/screen | Client, server, job and record survived; reattachment worked | Same; job kept the container alive and reattachment worked |
| Finite job after Docker detach | Job reached DONE, record cleared, container remained | Job reached DONE and container was removed |
| Codex login TUI after Docker detach or terminal loss | Codex processes and active record survived | Same; container remained running |

The waiting one-shot probe was explicitly sampled again after an additional
12-second wall-clock wait. The original dcc invocation had returned 1, while the
job and active record still existed. The orphan timeout applies before any command
has registered; it does not cancel an already-registered disconnected job.

On this Docker version, deliberate Docker detach produced `read escape sequence`
and the dcc invocation returned 1 (after its bounded teardown wait in the one-shot
case). This was a disconnect result, not evidence that the underlying job exited.
Closing the PTY caused local dcc to exit via SIGHUP (-1 in Python); killing dcc
returned -9. These local client outcomes did not terminate the observed jobs.

## Reconnection and explicit recovery

For tmux, reattached using a new dcc exec running `tmux attach-session -d -t probe`.
For screen, used `screen -D -r probe`. Force-detaching the previous mux client was
necessary to support cases where Docker detachment had left it attached. In every
surviving mux case, a line sent through the new terminal appeared in the original
probe's event log. This demonstrates resumed input to the existing job, not a new
probe invocation. Mux-own-detach one-shot cases had no container left to reconnect.

A follow-up durable tmux test sent quit after successful reattachment; the job
ended, records cleared, and the durable container remained. Separate Docker execs
could also TERM the original disconnected shell probe PID: the probe recorded TERM,
the wrapper cleaned its record, durable containers remained, and one-shot containers
were removed. These were recoverable live jobs, not stale records for dead PIDs.

A new ordinary exec is a new session; these experiments do not provide direct
reattachment to the old plain Docker exec terminal. Mux reconnection depends on
keeping its server's container alive. Container reset remains available when
individual-job recovery is unnecessary.

## Interpretation and limits

- Loss of a client is not equivalent to command completion or cancellation. The
  observed live-job records must not be reclaimed merely because the client died.
- One-shot lifetime follows registered wrapper lifetime. Mux servers outlive their
  clients, so mux-own-detach can deliberately remove the last registration while
  leaving useful server work; one-shot teardown then kills that work. Durable mode
  is needed if mux sessions should survive their normal detach operation.
- A finite disconnected job can finish and clean itself up. An interactive job
  waiting for input can remain indefinitely without user reconnection or explicit
  termination. The experiments establish survival during the observation windows,
  not a universal promise about all programs or indefinite runtime.
- Codex coverage was limited to its sign-in/onboarding TUI, with actual processes
  inspected before and after disconnect. Authenticated sessions, active model/tool
  work, and Codex inside a multiplexer were not tested.
- PTY closure, local process death and deliberate detach were tested; remote daemon
  network partition, SSH transport loss, host reboot and macOS/Desktop were not.
- This does not fix either confirmed race, and pre-command CLI-disconnection
  detection remains deferred. These experiments concern already-running commands.

## Evidence and cleanup

35 completed trials across the main, recovery, corrected finite-job, and explicit
12-second observation batches. Two initial Docker-client-kill attempts selected by
container name found no matching host client because dcc uses the immutable container
ID in exec arguments; they were harness failures and were rerun successfully with
exact-ID selection. An initial finite-job batch sampled too early because terminal
EOF shortened the pump wait; it was excluded and repeated using wall-clock waits.

Temporary scripts:
- /tmp/dcc-0113-disconnects.py
- /tmp/dcc-0113-disconnect-followup.py
- /tmp/dcc-0113-finite-disconnect.py
- /tmp/dcc-0113-detach-timeout.py

Temporary results.json files, process snapshots, probe logs, build logs and raw PTY
transcripts:
- /tmp/dcc-0113-disconnects-fyz9pgff (main; 26 successful trials plus two excluded selector failures)
- /tmp/dcc-0113-disconnects-39m7orrs (six recovery/follow-up trials)
- /tmp/dcc-0113-disconnects-fmlbhh3w (two corrected finite-job trials)
- /tmp/dcc-0113-disconnects-65llqtjp (explicit 12-second observation)

Each test container and its per-fixture image tags were removed after observation.
Temporary evidence and the reusable tools image remain for follow-up. No product
source changes were made; no product test suite was needed for these observations.
T-0113 remains open. This report preserves the method and results if /tmp expires.
