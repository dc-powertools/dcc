# T-0108 persistence quality record

Date: 2026-11-16. Route: Initiative. Risk: High. Owner: root.
Bounded implementation slices: T-0109 (dcc/Linux), T-0110 (local Codex pilot).
Parent T-0108 remains open.

## Readiness

Ready with concerns before implementation: frozen shared contract in execution notes,
nonoverlapping config/runtime/fixture/pilot ownership, independent review, no deletion,
rename recovery, no content scans. macOS/Desktop unavailable and blocks catalog rollout.
Rollback retains old roots, seed ledgers and displaced exclusion backups. Rebuild old
images with 0.3; do not restore old software into durable state automatically.

## Observed verification

| Check | Status | Evidence |
| --- | --- | --- |
| `cargo fmt --check` | Pass | Final integrated tree |
| `cargo clippy --all-targets -- -D warnings` | Pass | Final integrated tree |
| `cargo test --locked --quiet` | Pass | 572 unit and 124 runnable integration tests; 53 ignored explicitly documented |
| `cargo build --locked` | Pass | dcc 0.3.0 |
| Existing Docker smoke | Pass | 33/33, 138.89s; sudo Docker wrapper outside repository |
| New persistence suite | Pass | 10/10, 71.20s; final nonroot concurrency/mode/mount-guard source |
| Large durable data | Pass | 8GiB sparse + unreadable nested credential + FIFO: 17,408 KiB peak dcc RSS, 3.919s rebuild |
| Migration and recovery | Pass | Real package tree, broken/resolving stale links preserved once; auth/history/cache and malformed old ledger bytes retained |
| Empty/populated image exclusions, hooks, permissions | Pass | Synthetic local Feature; v1→v2; defaults only absent; existing-empty root retained; unwritable target fails |
| Concurrent first start/attachment | Pass | Six nonroot --keep launchers, one recovery tree; exact winning attempt observed, no startup replay |
| Schema/version compatibility | Pass | Strict new type, unknown-field rejection, substitutions/overlaps; old controller refresh gate; reseed rejects before Docker |
| Local Codex integration | Pass within stated bounds | Local pilot reference; 10 layout +2 Debian checks, concurrent daemon, rebuild/recreation; preferences/history/marketplace cache retained and package/tmp markers reset |
| Shellcheck | Pass | Modified fixture/pilot shell scripts checked in koalaman/shellcheck container; no production Features src changed |
| Docker Desktop/macOS | Not run | No runner available; blocks dcc completion gate and broad rollout |
| Codex automatic updater recovery/authenticated remote control/cross-version daemon migration | Not run | Further package qualification required; no production rollout claim |

Linux environment: Docker 29.9.3 linux/arm64, classic overlay2; Amazon Linux 2023.12,
kernel 6.18.62. Plain Docker socket denied; temporary wrapper executes sudo -n docker.
Host storage/security backend unchanged. No test accessed real credentials/history.

## Review and negative controls

Independent agents reviewed preservation, compatibility, permissions and coverage.
Corrected findings: original-path marker escaping, symlinked managed cache ancestry,
mount-target masking/aliases (including mounted durable aliases), concurrent name
ownership during created→running, nonroot mode promotion under Linux sticky-file
protections, and no-clobber staging. Fresh-only chown uses no-follow semantics.
Migration retains backups indefinitely; explicit manual recovery/retention is documented.

The strengthened six-process regression failed before the winner/mode fixes and passes
after. Masking regressions assert rejection before any durable initialization;
image/scratch reset and large-state canaries exercise behavior rather than merely
parser shape. A synthetic
hook helper initially raced on shared probe filenames; unique mktemp probes fixed the
fixture without weakening the concurrency criterion.

One early full unit run hit existing
`features::context::tests::generated_feature_install_scripts_execute_with_their_interpreter_contract`
with ETXTBSY. An independent inspection found std::fs::write closes before exec and
unique temp paths; no proven product/writer-handle cause. Subsequent full unchanged
runs passed. This remains a recorded intermittent test-environment risk owned by
T-0108 follow-up; no speculative retry/sleep or interpreter-semantic change was added.
A final boundary test fixture was updated to provide the compatible version label
required by the new refresh gate; separate negative tests cover legacy refresh/reseed.

## Batch and residual risk

Shared config StateEntry, build/runtime plumbing, schema and regression changes stay
in one coherent Linux contract commit because each depends on the new wire semantics.
Features pilot stays in its own repository commit outside publishable src. No release,
publication, push or host backend change. Live Desktop filesystem/SQLite behavior and
remaining catalog/updater gates are unresolved. Reuse retains frozen runtime config;
manual replacement of exclusion links during a running container is not automatically
repaired, and requires deliberate recovery/recreation. Runtime upgrades are best effort
and ephemeral; container recreation is the supported reset and rollback boundary.

## Completion

T-0109 Linux implementation: verified. T-0110 local pilot: verified and committed
separately. Parent T-0108: Needs verification; no broad Feature migration.
Exact-revision local-reference smoke passed with clean dcc source
`0331498d5fe4d86125ec692e9907426b3b71a59f` and Features
`6193b82c5d5ed49678858261ec69d079ce40d279`. Cached build/start, 10 pilot checks,
retained dummy preferences/history/plugin cache and daemon start/version/stop passed.
Immutable image: `sha256:5293f5feec835c314ddc711513646f46961d46fd76d06d9a099e922aa2e2afd3`.
Frozen binary SHA256: `35bf36c0e808c50f86426a4df5b4529b9158832af4a758766f8f7e801b9f3b6e`.
Runtime stopped; subsequent dcc record commit changes documentation only.
