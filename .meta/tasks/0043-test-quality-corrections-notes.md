# T-0043 Notes: Test Quality Corrections

## Checkpoint

- Status: Active.
- Completed slice: T-0044 (`inject_trace` removed; install scripts are byte-preserved;
  execution and no-secret-output tests include a tracing negative control).
- Completed slice: T-0048 (deterministic registry auth/status fixture, strict digest
  verification, metadata errors, and archive path/type confinement).
- Completed slice: T-0045 (Linux-only injectable UID planning and observed host-ID
  assertions; macOS/Windows simulations no-op).
- Completed slice: T-0046 (anonymous Feature volumes omit `source`; named volumes and
  bind mounts preserve it).
- Completed slice: T-0047 (two-way relay with half-close response drain, transactional
  listener binds, optional IPv6, retained/cancelled connection tasks, and connector
  cleanup; real Docker/`nc` smoke added but not runnable without Docker).
- Completed slice: T-0049 (successful path-profile identity comparison, genuinely
  Docker-free seeding dry-run, instance-identity reuse checks, and direct bounded
  teardown/removal assertions; retired bookkeeping checks removed).
- Completed slice: T-0050 (compiled-CLI fake Docker boundary for version gates,
  upstream pull policy, image/argument ordering, and optional resource limits).
- Completed slice: T-0051 (upstream-compatible absent/default/present-empty
  `containerEnv` contract, durable decision 0005, and post-substitution consumer
  validation coverage).
- Selected slice: T-0052, followed by T-0053.
- Integration owner: Root Orchestrator; one child task is selected and committed at a
  time.
- Delegation constraint: the requested fresh-agent sequence cannot start because this
  Codex session has neither the repository `codex-quota-monitor` skill nor another
  authoritative five-hour and weekly usage surface. Under the framework usage-capacity
  guard, capacity is unknown and child spawning is paused. The primary session is
  executing the same task-isolated sequence instead.
- Shared-tree baseline: task-catalog intake and T-0043 through T-0053 briefs were
  present as uncommitted framework state when implementation began; preserve unrelated
  task intake at every child commit.

## Verification Strategy

- Each child receives focused counterfactual or negative-control evidence where
  practical, focused tests, format/lint/build gates proportional to its risk, diff
  review, a task-scoped catalog result, and a local commit.
- The parent closes only after all children are Done and the aggregate format, clippy,
  test, build, Docker-smoke availability, consistency, and security reviews are
  recorded in the quality record.

## Issues To Present At Final Handoff

- Docker is not installed in this execution environment. T-0047's ignored live
  `docker exec -i ... nc` smoke compiles and its deterministic relay boundary passes,
  but the live smoke requires CI or another Docker-capable host.
