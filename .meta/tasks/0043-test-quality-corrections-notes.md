# T-0043 Notes: Test Quality Corrections

## Checkpoint

- Status: Done.
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
- Completed slice: T-0052 (candidate-by-candidate brittle-test classification, nine
  low-value tests removed, and three stable-outcome rewrites).
- Completed slice: T-0053 (fixed-seed merge/shell properties plus Dockerfile, seed
  digest, and Feature-edit behavior matrices).
- Aggregate closeout: all runnable gates, security/consistency review, and scheduled
  framework hygiene passed; no task remains selected.
- Integration owner: Root Orchestrator; one child task is selected and committed at a
  time.
- Shared-tree baseline: task-catalog intake and T-0043 through T-0053 briefs were
  present as uncommitted framework state when implementation began; preserve unrelated
  task intake at every child commit.

## T-0052 Candidate Classification

| Candidate | Disposition | Stable coverage retained or added |
| --- | --- | --- |
| Supervisor `REAPER_SECS`, `arrived`, retired grace/primed names, and exact `sleep 0.2` | Delete shape tests | Real-shell one-shot drain, bootstrap wait, failure status, and command-exit tests execute the scripts and observe behavior. |
| `generated_assets_returns_only_supervisor_scripts` exact length and retired hook path | Delete forwarding-layer duplicate; rewrite the owning supervisor asset test | The owning test requires the supervisor, control, and exec assets by public image path and verifies every emitted asset is executable shell. |
| Retired stop dry-run phrase | Already deleted by T-0049 | `stop_dry_run_reports_action_for_each_variant` positively checks the graceful, now, and kill actions. |
| Root remap test titled as planned while passing `None` | Delete duplicate/misleading test | UID planning tests prove root never produces a plan; the generator's no-plan test proves omission. |
| Exact version-label placement and exact root Dockerfile body | Delete placement test; rewrite root test | Root-user creation is absent and the required version label is present without fixing line position or whitespace. |
| Netcat last-line ordering with no Features | Rewrite | A Feature fixture proves its install step precedes netcat installation; separate tests retain package alternatives, user ordering, and omission. |
| Duplicate workspace-root example | Delete `root_is_workspace` | `from_workspace_root` observes the same exact canonical root; nested and `.devcontainer` discovery remain separate. |
| Cache absolute-path example | Delete | `host_path_correct` asserts the stronger exact cache path, while creation/idempotence tests use a real absolute temp root. |
| Container-id prefix and profile-suffix fragments | Delete | Exact-result ID tests cover both format parts together, with separate identity stability and distinction cases. |

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
