# T-0104: Concurrent Codex Run and Attach

2026-11-16. User requested `dcc run codex:yolo` followed by `dcc attach`
while the first session remains active. Used the T-0103 isolated fixture at
`/tmp/dcc-codex-evaluation`, dcc 0.2.1, Debian bookworm ARM64, published Codex
feature and Codex CLI 0.169.0, Docker 29.9.3. Commands used the existing debug
binary via `sg docker -c`; no product or fixture configuration changes.

## Verification

1. With no existing containers, launched `dcc run codex:yolo` in a real PTY.
   The feature command resolved to `codex resume --yolo`. Codex opened its welcome
   and sign-in screen and remained active.
2. Docker reported container `f12be140e3e6`, named `Codex-evaluation`, running.
3. In a second real PTY, launched `dcc attach` from the same workspace.
   It immediately opened a shell as dev in `/workspace`.
4. In that shell, `hostname` returned `f12be140e3e6`; `id` reported uid 1000(dev)
   and gid 993(dev). Process inspection showed the original `codex resume --yolo`
   process (PID 50), its dcc-exec wrapper, and its managed app-server and PID-update
   processes. `codex --version` returned `codex-cli 0.169.0`, and the shell printed
   `CONCURRENT_ATTACH_OK`.
5. Exited the attach shell successfully. The original run PTY remained active and
   Docker still reported the same running container.
6. Exited Codex using Ctrl-C (run returned exit 0), then ran `dcc stop` successfully.
   Final Docker container list was empty.

## Result and limits

PASS: interactive attachment to the running profile succeeds while the Codex yolo
command is active; both sessions share one container. Exiting the attach shell does
not terminate the original session. No authenticated model request was attempted:
Codex remained at its sign-in screen. Fixture has no forwarded ports, so this does
not independently test concurrent forwarding behavior. The seed tag/digest warning
from T-0103 also appeared on initial creation and did not block either session.

Focused evidence review and documentation whitespace check passed. No product code
changed. Test container removed.
