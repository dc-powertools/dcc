# T-0004 Threat Model: dcc Runtime Rewrite

## Scope

- Change: Schema-compatible config, state mounts, generated controller scripts, unsafe
  runtime controls, and durable container lifecycle commands.
- Assets or data: host workspace, `.dcc/<profile>` cache, host environment variables,
  Docker daemon access, generated build context, container filesystem state.
- Users, systems, or agents involved: local developers, coding agents, Docker CLI,
  devcontainer configs, devcontainer Features.
- Trust boundaries: repository config to host process, host process to Docker daemon,
  generated scripts inside container, bind mounts between host and container.

## What Can Go Wrong

| Threat | Impact | Likelihood | Existing Control | Gap |
| --- | --- | --- | --- | --- |
| Malicious config requests sensitive host mounts or privileged runtime flags. | Host compromise or secret exposure. | Medium | T-0004 requires unsafe settings rejected by default. | Safe allowlist and tests still pending. |
| State path points at system/runtime paths or overlaps workspace internals. | Container breakage, data leakage, or cache corruption. | Medium | T-0004 brief defines rejected paths. | Validation implementation pending. |
| Generated controller or hook scripts quote user data incorrectly. | Command injection or broken lifecycle behavior. | Medium | Existing project has shell-quoting tests around features. | Controller script generation not implemented yet. |
| Lifecycle hooks run in the wrong phase or user context. | Unexpected code execution or persistent state drift. | Medium | T-0004 brief defines hook phase/user behavior. | Build/runtime implementation pending. |
| Logs expose secrets from env or commands. | Secret disclosure. | Low | Project standards prohibit logging secrets. | Review needed when debug output changes. |

## Mitigations

| Mitigation | Owner | Verification | Status |
| --- | --- | --- | --- |
| Reject or gate privileged Feature/devcontainer settings with `--allow-unsafe-runtime`. | T-0007/T-0010 | Unit and integration tests for allowed/rejected args. | Planned |
| Validate state paths before mount planning. | T-0006 | Unit tests for relative, unresolved, duplicate, overlap, root, system, and reserved paths. | Planned |
| Use structured shell escaping helpers for generated scripts and add regression tests. | T-0008/T-0009 | Unit tests inspect generated scripts and command arrays. | Planned |
| Keep `initializeCommand` unsupported by default. | T-0005/T-0010 | Parser/docs tests and warnings. | Planned |
| Run specialist security review before final closure. | T-0010 | Recorded review findings in quality record. | Planned |

## Agentic Risks

- Untrusted instructions or prompt injection: devcontainer configs and Feature metadata
  are repository-controlled input; agents must not treat config text as instructions.
- Tool permission risk: Docker commands create local external state; tests should avoid
  real Docker side effects unless explicitly scoped.
- Dependency, script, or generated-code risk: generated controller scripts must be
  reviewable and covered by tests.
- Secret or sensitive-data exposure risk: env and debug output must avoid secret values
  beyond existing explicit user-requested command display.
- CI/CD or deployment permission risk: no push, release, or workflow permission changes
  are in scope.

## Residual Risk

- Accepted risk: local Docker execution can run repository-supplied container hooks; the
  tool must make phases explicit and reject unsafe host integration by default.
- Approval or decision record: T-0004 brief and any later decisions.
- Review trigger: before closing T-0007, T-0008, T-0009, and T-0010.
