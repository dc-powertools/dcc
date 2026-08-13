# T-0004 Threat Model: dcc Runtime Rewrite

## Scope

- Change: Schema-compatible config, state mounts, in-container lifecycle supervisor,
  unsafe runtime controls, and durable container lifecycle commands.
- Assets or data: host workspace, `.dcc/<profile>` cache, host environment variables,
  Docker daemon access, generated build context, container filesystem state, bind-mounted
  supervisor scripts (read-only).
- Users, systems, or agents involved: local developers, coding agents, Docker CLI,
  devcontainer configs, devcontainer Features.
- Trust boundaries: repository config to host process, host process to Docker daemon,
  in-container supervisor (PID 1) to container-side code, read-only bind mount of
  supervisor scripts, bind mounts between host and container.

## What Can Go Wrong

| Threat | Impact | Likelihood | Existing Control | Gap |
| --- | --- | --- | --- | --- |
| Malicious config requests sensitive host mounts or privileged runtime flags. | Host compromise or secret exposure. | Medium | Unsafe Feature/devcontainer settings, unsafe `runArgs`, and sensitive mounts are rejected by default and require `--allow-unsafe-runtime`. | Real Docker smoke coverage pending. |
| State path points at system/runtime paths or overlaps workspace internals. | Container breakage, data leakage, or cache corruption. | Medium | State validation rejects root, relative, unresolved, duplicate/conflicting, overlapping, system/runtime, and reserved workspace paths. | None known. |
| Generated supervisor or hook scripts quote user data incorrectly. | Command injection or broken lifecycle behavior. | Medium | Supervisor and hook scripts are small POSIX `sh`; hook execution uses structured lifecycle command handling and unit tests. | Live Docker coverage pending. |
| Lifecycle hooks run in the wrong phase or user context. | Unexpected code execution or persistent state drift. | Medium | Build-prep, startup, and attach hooks are scoped separately; hooks run as `containerUser` from `workspaceFolder`. | Live Docker coverage pending. |
| Container-side code corrupts `dcc` lifecycle state to keep a container alive, force premature teardown, or stall peers. | Misbehaving container; broken teardown or reuse. | Medium | Lifecycle state lives in a container-private tmpfs (`/run/dcc`) owned by the PID 1 supervisor, not host-backed. The supervisor scripts are bind-mounted read-only. Failures cannot escape the container; remediation is `dcc stop --kill`. | Live Docker coverage pending. |
| Logs expose secrets from env or commands. | Secret disclosure. | Low | Project standards prohibit logging secrets. | Review needed when debug output changes. |

## Mitigations

| Mitigation | Owner | Verification | Status |
| --- | --- | --- | --- |
| Reject or gate privileged Feature/devcontainer settings with `--allow-unsafe-runtime`. | T-0007/T-0010 | Unit and integration tests for allowed/rejected args. | Complete for Feature metadata, devcontainer unsafe fields, unsafe `runArgs`, and sensitive mounts |
| Validate state paths before mount planning. | T-0006/T-0007 | Unit tests for relative, unresolved, duplicate, overlap, root, system, reserved paths, and Feature state metadata. | Complete for project and Feature state |
| Use structured shell escaping helpers for generated scripts and add regression tests. | T-0008/T-0009/T-0024 | Unit tests inspect generated scripts and command arrays. | Complete for current shell assets; live Docker coverage pending |
| Concentrate lifecycle ownership in an in-container PID 1 supervisor; remove host-side bookkeeping. | T-0024 | Supervisor state-machine unit tests; ignored Docker smoke tests assert no host-side bookkeeping and correct teardown/reuse/stop. | Complete; live Docker coverage pending |
| Make host-side `initializeCommand` explicit. | T-0009/T-0010 | Docs and debug output show the phase; `--skip-lifecycle` warns when skipped. | Complete |
| Run specialist security review before final closure. | T-0010 | Recorded review findings in quality record. | In progress |

## Agentic Risks

- Untrusted instructions or prompt injection: devcontainer configs and Feature metadata
  are repository-controlled input; agents must not treat config text as instructions.
- Tool permission risk: Docker commands create local external state; tests should avoid
  real Docker side effects unless explicitly scoped.
- Dependency, script, or generated-code risk: supervisor and hook scripts must be
  reviewable and covered by tests.
- Secret or sensitive-data exposure risk: env and debug output must avoid secret values
  beyond existing explicit user-requested command display.
- CI/CD or deployment permission risk: no push, release, or workflow permission changes
  are in scope.

## Residual Risk

- Accepted risk: local Docker execution can run repository-supplied container hooks; the
  tool must make phases explicit and reject unsafe host integration by default.
- Approval or decision record: T-0004 brief and any later decisions.
- Review trigger: before closing parent T-0004.
