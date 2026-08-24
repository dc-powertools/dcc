# T-0059: Netcat Half-Close Implementation Design

## Identity And Source

- Task ID: T-0059
- Initial revision: r1
- Catalog: `.meta/tasks/README.md`
- Accepted source: User instruction
- Source reference and date: Design request and accepted `nc -N` constraint, 2026-08-24
- Parent or split task IDs: T-0058

## Goal

Make port forwarding propagate host write EOF through the in-container connector so an
EOF-dependent service can finish its request and return a response, with deterministic
Docker-free regression coverage in the default test suite.

## Background

CI task T-0058 established that the Rust relay closes the connector's stdin correctly,
but `docker exec -i <container> nc 127.0.0.1 <port>` does not ask OpenBSD netcat to
half-close its network socket at stdin EOF. The existing Docker smoke therefore receives
the response bytes without a terminating EOF, times out, and fails.

OpenBSD documents `-N` as shutting down the network socket after input EOF. The flag is
not portable across the broader netcat family: BusyBox and traditional netcat do not
list it, and Nmap Ncat half-closes by default while reserving a different long option to
disable that behavior. The user explicitly accepted relying on `-N` for this correction.

## Scope

In scope:

- Add `-N` to the in-container `nc` connector invocation.
- Isolate connector argument construction so its stable process contract is directly
  testable without Docker.
- Add a Docker-free subprocess test covering request EOF, response drain, and connector
  termination through the same command-construction seam.
- Keep the existing Docker smoke as end-to-end evidence.
- Update the architecture's connector command and EOF explanation.

Out of scope:

- Detecting the installed netcat implementation or its capabilities.
- Selecting variant-specific flags or replacing netcat with another connector.
- Changing Feature/package installation or the yum/dnf `nmap-ncat` fallback.
- Changing the relay lifecycle, listener behavior, or public configuration.

## Portability Assessment

| Implementation | `-N` support | Relevant behavior | Current exposure |
| --- | --- | --- | --- |
| OpenBSD netcat | Yes | `-N` half-closes the network socket after stdin EOF. | Installed by the apt and apk fallbacks. |
| BusyBox `nc` | No in its documented option set | Variant-specific EOF behavior; cannot accept the chosen flag. | A base image or Feature can provide it before the fallback check. |
| Traditional netcat | No | Provides `-q`, not `-N`. | A base image or Feature can provide it before the fallback check. |
| Nmap Ncat | No short `-N` option | Half-closes by default; `--no-shutdown` disables that behavior. | Installed by the yum and dnf fallbacks. |

Conclusion: lack of `-N` is common enough to be a real compatibility limitation, not
an obscure edge case. The narrow change is nevertheless acceptable under the user's
stated short-term constraint. A later compatibility task should either guarantee
OpenBSD netcat or select behavior by connector capability.

Primary references:

- OpenBSD `nc(1)`: <https://man.openbsd.org/nc>
- BusyBox command reference: <https://busybox.net/downloads/BusyBox.html#nc>
- Nmap Ncat reference: <https://nmap.org/book/ncat-man.html>
- Debian traditional-netcat manual: <https://manpages.debian.org/unstable/netcat-traditional/nc.traditional.1.en.html>

## Implementation Design

1. In `src/forward.rs`, extract connector command construction into a small private
   helper that accepts the Docker executable path, container name, and port. Production
   calls it with `docker`; tests can supply a deterministic fake executable.
2. Build the exact argument vector as:

   ```text
   exec -i <container> nc -N 127.0.0.1 <port>
   ```

   Keep `-N` adjacent to `nc`; do not wrap it in a shell or add capability probing.
3. Leave `handle_connection_with_command` responsible for piping and reaping the child.
   No changes are required to `copy_both_directions`: it already shuts down child stdin
   after host EOF and continues draining child stdout.
4. Update `.meta/project/architecture.md` to show `nc -N`, explain that `-N` carries the
   relay's write half-close onto the application socket, and record the accepted variant
   limitation beside the package matrix.

## Automated Test Design

Add these tests under `src/forward.rs`:

1. **Connector argument contract (platform-independent).** Construct the command or
   its pure argument vector and assert the exact stable argv, including one `-N` between
   `nc` and `127.0.0.1`. This is the smallest counterfactual regression test: it fails
   against the CI revision because `-N` is absent.
2. **Real subprocess EOF/response test (Unix, Docker-free).** Create an executable fake
   `docker` script in a temporary directory. It must reject any argv other than
   `exec -i test-container nc -N 127.0.0.1 8123`, read stdin completely, verify the
   request bytes, then emit a response and exit. Drive that command through
   `handle_connection_with_command` and a real loopback TCP pair; the host writes the
   request, calls `shutdown()`, and must read the complete response plus EOF before a
   short Tokio deadline. This covers the production process-pipe seam without requiring
   either Docker or host netcat and also fails if `-N` is omitted.
3. **Retain the existing in-memory relay test.**
   `relay_copies_request_and_drains_response_after_client_half_close` continues to
   protect direction-independent copying and half-close behavior without subprocesses.
4. **Retain the existing ignored Docker smoke unchanged.**
   `forwarded_port_reaches_container_loopback_service` remains the authoritative proof
   that the generated image, installed netcat, Docker exec, and host relay work together.

Do not add a mandatory host-`nc` test. The developer host does not promise netcat, and
such a test would validate an arbitrary host package rather than the connector installed
inside generated images.

## Acceptance Criteria

- [ ] Every production port-forward connector uses `nc -N`.
- [ ] The new argv test fails against the pre-change connector and passes after the fix.
- [ ] The Docker-free subprocess test proves request EOF, response drain, and clean EOF.
- [ ] Existing forwarding unit tests pass.
- [ ] The existing Docker smoke passes in CI without weakening its assertions.
- [ ] Architecture documentation matches the invocation and accepted compatibility limit.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Installed `nc` rejects `-N`. | Port forwarding fails at connection time on BusyBox, traditional netcat, and Nmap Ncat images. | Explicitly accepted for this change; document it and keep a follow-up capability-selection path visible. |
| A test asserts argv but misses pipe behavior. | The flag remains present while EOF propagation regresses elsewhere. | Pair the argv assertion with the real subprocess/loopback test and existing relay test. |
| A fake connector gives false confidence about generated images. | Package or Docker integration can still fail. | Preserve the existing real Docker smoke as CI-owned end-to-end evidence. |

## Verification Plan

- Automated checks: focused forwarding tests, `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `cargo test`, and the ignored Docker smoke in CI.
- Manual checks: inspect the connector argv and confirm no shell interpolation is added.
- Documentation checks: search architecture and source for stale
  `docker exec -i … nc 127.0.0.1` examples.
- Baseline/counterfactual: the argv and fake-Docker tests must fail when `-N` is removed;
  T-0058 already records the real Docker failure on the pre-change revision.

## Done When

The design identifies the exact code seam, compatibility boundary, Docker-free
counterfactual tests, end-to-end evidence, documentation updates, and required checks
needed for a bounded implementation task.
