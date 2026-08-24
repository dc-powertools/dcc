# T-0060: Variant-Aware Port-Forward Connector

## Identity And Source

- Task ID: T-0060
- Initial revision: r1
- Current revision: r2
- Catalog: `.meta/tasks/README.md`
- Accepted source: User instruction
- Source reference and date: Implementation intake plus wrapper-scope expansion,
  2026-08-24
- Parent or split task IDs: T-0058 and T-0059

## Goal

Implement reliable port-forward write-half-close behavior without assuming that every
container's `nc` supports OpenBSD's `-N` flag. Hide supported netcat variants behind a
fixed in-container `dcc-connect` command and protect selection plus byte-flow behavior
with deterministic Docker-free tests.

## Background

T-0058 diagnosed the CI failure: the existing `docker exec -i … nc` connector does not
propagate stdin EOF to the application socket under OpenBSD netcat unless `-N` is used.
T-0059 initially designed the narrow `nc -N` correction and recorded that `-N` is not
portable. Revision r2 expands implementation to address that limitation:

- OpenBSD netcat supports and requires `-N` for the intended behavior.
- Nmap Ncat half-closes by default and does not accept short `-N`.
- BusyBox and traditional netcat do not provide the required `-N` interface.
- The current build accepts any pre-existing `nc`, while apt/apk install OpenBSD netcat
  and yum/dnf install Nmap Ncat.

This brief supersedes the narrow implementation scope in
`.meta/tasks/0059-nc-half-close-design.md`; that file remains the diagnostic and
portability basis.

## Scope

In scope:

- Bake an executable `/usr/local/share/dcc/dcc-connect` wrapper into built images.
- Make the host relay invoke only that fixed wrapper through `docker exec -i`.
- Select OpenBSD netcat with `-N` and Nmap Ncat without `-N`.
- Reject unknown or unsupported netcat variants with a clear build-time error.
- Replace the current `command -v nc` short circuit with compatibility validation and
  package-manager fallbacks that establish a supported connector.
- Add Docker-free variant-selection, argv, process EOF, response-drain, and unsupported
  variant tests.
- Retain and run the existing in-memory relay tests and unchanged Docker smoke.
- Align architecture documentation and generated-Dockerfile examples.

Out of scope:

- Supporting BusyBox or traditional netcat directly.
- Adding `-q 0` as a substitute; it terminates rather than preserving response drain
  after a write half-close.
- Shipping a new compiled connector binary.
- Changing public `forwardPorts` configuration or listener lifecycle behavior.

## Implementation Design

### Fixed connector boundary

Add `dcc-connect` to the generated assets copied into `/usr/local/share/dcc/`. The host
side in `src/forward.rs` must spawn:

```text
docker exec -i <container> /usr/local/share/dcc/dcc-connect 127.0.0.1 <port>
```

Keep connector command construction in a private injectable helper so tests can provide
a fake Docker executable without modifying global `PATH`.

### Wrapper selection

The POSIX shell wrapper accepts `HOST PORT` and a build-only `--check` mode. Use this
priority:

1. `nc.openbsd -N HOST PORT` when `nc.openbsd` exists.
2. `ncat HOST PORT` when Nmap Ncat exists and identifies itself successfully.
3. `nc -N HOST PORT` only when `nc -h` explicitly advertises a standalone `-N` option.
4. Otherwise print a concise unsupported-connector error and exit nonzero.

Selection must use direct argv with `exec`, validate the port as a decimal value in the
valid TCP range, and avoid `eval` or shell-built command strings. `--check` exercises the
same selection logic without opening a connection.

### Build provisioning

Update generated Dockerfile ordering so the wrapper is available for compatibility
checks after Feature installation. For non-empty `forwardPorts`:

1. Run `dcc-connect --check` and keep a compatible pre-existing connector.
2. If unsupported, install `netcat-openbsd` through apt/apk or `nmap-ncat` through
   yum/dnf, retaining the current package-manager order and noninteractive apt behavior.
3. Run `dcc-connect --check` again and fail the image build with actionable output if no
   supported connector is available.

Features must still install before connector provisioning, and all generated assets
must retain executable modes. An arbitrary `command -v nc` result is no longer enough
to short-circuit provisioning.

## Automated Test Design

### Wrapper unit/behavior matrix

Use temporary fake executables and an isolated `PATH` to prove:

- `nc.openbsd` is invoked with exactly `-N HOST PORT`.
- Nmap `ncat` is invoked with exactly `HOST PORT` and no `-N`.
- A generic `nc` is used only when its help advertises standalone `-N`, and receives it.
- BusyBox-like, traditional-like, missing, or unrecognized implementations fail with a
  sanitized, actionable error.
- Explicit OpenBSD takes precedence over Ncat, and Ncat takes precedence over generic
  probing, so selection is deterministic.
- Invalid host/port arity and invalid ports fail before invoking a connector.

### Host relay subprocess test

Create an executable fake Docker program that validates the production argv ending in
`/usr/local/share/dcc/dcc-connect 127.0.0.1 8123`. Drive it through
`handle_connection_with_command` and a real loopback TCP pair. The fake reads request
stdin to EOF, emits a response, and exits; the client must receive the response and final
EOF within a bounded deadline. This protects the real child-pipe boundary without
Docker or host netcat.

### Build-context tests

Assert that generated images with `forwardPorts`:

- contain executable `dcc-connect` bytes unchanged;
- install Features before connector compatibility/provisioning;
- invoke `--check` before and after fallback installation;
- retain apt/apk OpenBSD and yum/dnf Ncat fallbacks; and
- do not accept arbitrary `command -v nc` as sufficient.

Retain `relay_copies_request_and_drains_response_after_client_half_close` and the
unchanged ignored `forwarded_port_reaches_container_loopback_service` Docker smoke.
Do not require host netcat in the default suite.

## Acceptance Criteria

- [ ] Host forwarding invokes only the baked `dcc-connect` boundary.
- [ ] OpenBSD netcat receives `-N`; Nmap Ncat does not.
- [ ] Unknown, BusyBox, and traditional variants cannot silently satisfy provisioning.
- [ ] A compatible pre-existing connector avoids unnecessary package installation.
- [ ] Package fallbacks end with a successful compatibility check or a clear build error.
- [ ] Docker-free tests cover selection, exact argv, process EOF, response drain, and
      unsupported variants with counterfactual confidence.
- [ ] The unchanged Docker smoke passes in CI.
- [ ] Architecture documentation matches the wrapper, selection order, provisioning,
      and remaining compiled-helper alternative.

## Constraints And Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Help-text probing misidentifies generic `nc`. | The wrapper passes an unsupported flag or rejects a compatible binary. | Prefer explicit `nc.openbsd` and Ncat identities; require a standalone `-N` help entry for generic `nc`; cover representative outputs. |
| Wrapper and Dockerfile capability logic drift. | Build passes but runtime selection fails. | Make `dcc-connect --check` the single selector used before and after installation. |
| Generated asset ordering breaks Feature-provided connectors. | Extra packages install or a Feature connector is ignored. | Preserve Feature-before-provisioning order and test both compatible and unsupported Feature outcomes. |
| Shell argument handling creates injection risk. | Untrusted host/port input could execute shell syntax. | Use positional parameters, validate arity/port, direct `exec`, and no `eval`. Host remains the fixed literal `127.0.0.1` in production. |
| Docker-free fakes diverge from image packages. | Unit tests pass while package integration fails. | Keep the existing Docker smoke unchanged and CI-owned. |

## Verification Plan

- Focused tests: wrapper selection matrix, forwarding subprocess test, generated
  build-context tests, and existing `forward::tests`.
- Required repository checks: `cargo fmt --check`, `cargo check`,
  `cargo clippy -- -D warnings`, `cargo test`, `cargo build`, and `git diff --check`.
- Documentation checks: search for stale direct `docker exec -i … nc` and unconditional
  `command -v nc` descriptions.
- End-to-end: existing ignored Docker smoke in CI, unchanged.
- Counterfactual: remove the OpenBSD `-N` branch, add `-N` to Ncat, or allow arbitrary
  `nc`; the relevant Docker-free matrix case must fail.

## Material Amendments

| Revision | Date | Source | Change | Reason | Scope Or Acceptance Impact |
| --- | --- | --- | --- | --- | --- |
| r2 | 2026-08-24 | User follow-up | Replace the direct `nc -N` assumption with a variant-aware baked wrapper and compatible provisioning. | `-N` is absent from several common netcat families. | Adds generated assets, build provisioning, selection/error behavior, and a broader test matrix while preserving the original EOF outcome. |

## Done When

The fixed wrapper boundary selects only proven half-close behavior, generated images
establish a supported connector or fail clearly, Docker-free counterfactual coverage
passes, the unchanged Docker smoke is ready for CI, documentation is aligned, and every
runnable required check passes.
