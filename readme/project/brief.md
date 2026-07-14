# Project Brief

## Project

- Name: dcc
- One-sentence purpose: A Rust CLI that wraps Docker to manage profile-specific
  devcontainer environments with one-shot or durable lifecycle modes, durable
  per-profile caches, config inheritance, and devcontainer Feature installation.
- Current stage: Established CLI project.
- Primary repository or service: This repository.
- Last updated: 2026-07-14

## Users And Stakeholders

| Group | Need | Current Pain | Success Signal |
| --- | --- | --- | --- |
| Developers using devcontainers | Start isolated profile-specific environments quickly. | Long-lived containers and shared state can make agentic coding sessions hard to reset safely. | Users can build, run, exec into, and stop profile-specific devcontainers predictably. |
| Maintainers | Keep the CLI reliable, testable, and releaseable. | Docker/devcontainer behavior has many edge cases around config, users, environment, mounts, and lifecycle hooks. | CI and local verification pass; documented behavior matches the binary. |

## Outcomes

- Business outcome: Provide a small installable CLI for repeatable devcontainer workflows
  on macOS and Linux.
- User outcome: Make spinning up and tearing down isolated development environments easy,
  automatic, and safe.
- Operator or maintainer outcome: Keep behavior covered by Rust tests, clear architecture
  notes, and reproducible local commands.
- Non-goals: No library API is planned; the crate is consumed as a single binary.

## Core Workflows

| Workflow | Actor | Trigger | Expected Result | Notes |
| --- | --- | --- | --- | --- |
| Install CLI | User | Runs the installation script from the public README. | `dcc` is installed under `~/.local/bin`. | Requires Docker for real devcontainer use. |
| Build profile image | User or agent | Runs `dcc build` for a profile. | The profile config resolves and a local Docker image is prepared. | `--strict` turns unknown config fields into errors. |
| Run profile container | User or agent | Runs `dcc run`, `dcc exec`, `dcc start`, or `dcc attach` for a profile. | A one-shot or durable container starts with cache, mounts, env, lifecycle hooks, and command handling applied. | `dcc build` must be run first. |
| Maintain the CLI | Maintainer or agent | Changes Rust code or docs. | Required local checks pass before completion. | Canonical commands live in `readme/project/standards.md`. |

## Constraints

- Technical: Rust 2021 single binary crate; Docker CLI is the runtime integration.
- Product: Linux and macOS are the documented target platforms.
- Legal or compliance: None recorded.
- Security or privacy: Do not log secrets, tokens, or credentials.
- Performance or reliability: Preserve profile isolation and deterministic config
  resolution; release builds enable overflow checks.
- Budget or time: None recorded.

## Acceptance Defaults

- Definition of done: Satisfy the requested behavior, update relevant docs, and pass all
  required runnable checks.
- Required checks: Use the verified command catalog in `readme/project/standards.md`.
- Required docs: Update project memory, README, architecture, or style records when the
  behavior or durable convention changes.
- Release or demo expectations: Do not release or push without explicit owner direction.

## Canonical Knowledge Links

- Open assumptions: None
- Glossary: `readme/project/glossary.md`
- Source map: `readme/project/source-map.md`
- Project context: `readme/project/context.md`
