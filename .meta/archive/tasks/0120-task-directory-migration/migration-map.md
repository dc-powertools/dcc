# Migration inventory and path mapping

Baseline: `5ffbb1d034a72f7bcb88a5c562f88b966c6933ce` (2026-11-18). The worktree was clean before execution.

126 tasks: 119 Done, one Superseded, six nonterminal including this migration. 52 rows have no linked detail. Six legacy T-0024 subtasks retain their complete IDs. 59 task details, 31 quality records, ten independently numbered decisions, five threat models, and one incident are preserved. No optional harness adapters or pre-existing archives were present.

Ownership was reviewed using catalog links and artifact content before moving files. Decision numbers are independent of task numbers. The supervisor research subtask owns its design; its parent links to it. Shared reports have one owner and all catalog consumers link to that file. T-0108 owns its cross-slice execution/quality/contract evidence; T-0109 and T-0110 retain explicit shared links. T-0113 owns interactive-disconnect research shared with T-0115. T-0070 owns the cross-slice TLS threat model; T-0071 owns the CA decision.

## Task records

| Legacy ID | Canonical README |
| --- | --- |
| T-0001 | `.meta/archive/tasks/0001-load-agent-files-and-initialize-the-development-framework/README.md` |
| T-0002 | `.meta/archive/tasks/0002-evaluate-agents-bak-md-and-unexpected-readme-files/README.md` |
| T-0003 | `.meta/archive/tasks/0003-migrate-readme-development-md-readme-style-md-and/README.md` |
| T-0004 | `.meta/archive/tasks/0004-dcc-rewrite/README.md` |
| T-0005 | `.meta/archive/tasks/0005-config-schema/README.md` |
| T-0006 | `.meta/archive/tasks/0006-state-cache/README.md` |
| T-0007 | `.meta/archive/tasks/0007-feature-metadata/README.md` |
| T-0008 | `.meta/archive/tasks/0008-build-prep/README.md` |
| T-0009 | `.meta/archive/tasks/0009-runtime-lifecycle/README.md` |
| T-0010 | `.meta/archive/tasks/0010-final-validation/README.md` |
| T-0011 | `.meta/archive/tasks/0011-install-missing-validation-tooling-and-run-official-dev/README.md` |
| T-0012 | `.meta/archive/tasks/0012-enable-docker-dependent-smoke-tests-in-github-actions/README.md` |
| T-0013 | `.meta/archive/tasks/0013-add-docker-free-dry-run-validation-and-json/README.md` |
| T-0014 | `.meta/archive/tasks/0014-add-expanded-docker-behavior-smoke-tests-for-the/README.md` |
| T-0015 | `.meta/archive/tasks/0015-update-user-facing-documentation-to-match-the-completed/README.md` |
| T-0016 | `.meta/archive/tasks/0016-remove-initializecommand-execution-and-keep-it-parse-and/README.md` |
| T-0017 | `.meta/archive/tasks/0017-reorganize-readme-lifecycle-hook-guidance-into-a-dedicated/README.md` |
| T-0018 | `.meta/archive/tasks/0018-move-shared-readme-command-flag-guidance-into-a/README.md` |
| T-0019 | `.meta/archive/tasks/0019-make-debug-a-global-cli-flag-supported-by/README.md` |
| T-0020 | `.meta/archive/tasks/0020-design-and-implement-a-feature-subcommand-that-accepts/README.md` |
| T-0021 | `.meta/archive/tasks/0021-state-path-guards/README.md` |
| T-0022 | `.meta/archive/tasks/0022-state-seeding/README.md` |
| T-0023 | `.meta/archive/tasks/0023-cache-runtime-exposure/README.md` |
| T-0024 | `.meta/archive/tasks/0024-supervisor-lifecycle-ownership/README.md` |
| T-0024-R1 | `.meta/archive/tasks/0024-r1-r1-supervisor-design/README.md` |
| T-0024-S1 | `.meta/archive/tasks/0024-s1-implement-the-pid-1-supervisor-and-command-entrypoint/README.md` |
| T-0024-H1 | `.meta/archive/tasks/0024-h1-rework-the-host-cli-src-exec-rs-src/README.md` |
| T-0024-B1 | `.meta/archive/tasks/0024-b1-update-the-build-preparation-container-src-build-rs/README.md` |
| T-0024-T1 | `.meta/archive/tasks/0024-t1-add-tests-supervisor-state-machine-unit-tests-entrypoint/README.md` |
| T-0024-D1 | `.meta/archive/tasks/0024-d1-update-meta-threat-models-0004-dcc-runtime-md/README.md` |
| T-0025 | `.meta/archive/tasks/0025-supervisor-startup-handshake/README.md` |
| T-0026 | `.meta/archive/tasks/0026-update-remote-user-uid/README.md` |
| T-0027 | `.meta/archive/tasks/0027-remove-image-fast-path/README.md` |
| T-0028 | `.meta/archive/tasks/0028-supervisor-delivery-model/README.md` |
| T-0029 | `.meta/archive/tasks/0029-bake-the-supervisor-scripts-dcc-supervisor-dcc-ctl/README.md` |
| T-0030 | `.meta/archive/tasks/0030-replace-the-dcc-version-mismatch-warning-with-a/README.md` |
| T-0031 | `.meta/archive/tasks/0031-remove-the-vestigial-build-prep-hook-sh-assets/README.md` |
| T-0033 | `.meta/archive/tasks/0033-relocate-everything-under-the-readme-directory-to-meta/README.md` |
| T-0032 | `.meta/archive/tasks/0032-whether-to-bake-hooks/README.md` |
| T-0034 | `.meta/archive/tasks/0034-diagnose-the-latest-ci-failure-from-logs-86711534614/README.md` |
| T-0035 | `.meta/archive/tasks/0035-update-the-commit-trailer-requirement-to-name-the/README.md` |
| T-0036 | `.meta/archive/tasks/0036-diagnose-the-latest-ci-failure-from-logs-86723951441/README.md` |
| T-0037 | `.meta/archive/tasks/0037-diagnose-the-latest-ci-failure-from-logs-86726174884/README.md` |
| T-0038 | `.meta/archive/tasks/0038-diagnose-the-latest-ci-failure-from-logs-86727797288/README.md` |
| T-0039 | `.meta/archive/tasks/0039-reorganize-public-documentation-keep-readme-md-as-an/README.md` |
| T-0040 | `.meta/archive/tasks/0040-reorganize-public-docs-again-move-the-broad-usage/README.md` |
| T-0041 | `.meta/archive/tasks/0041-remove-lockfile-implementation-because-feature-backed-installs-are/README.md` |
| T-0042 | `.meta/archive/tasks/0042-fully-remove-the-dcc-build-update-flag-and/README.md` |
| T-0043 | `.meta/archive/tasks/0043-test-quality-corrections/README.md` |
| T-0044 | `.meta/archive/tasks/0044-feature-install-tracing/README.md` |
| T-0045 | `.meta/archive/tasks/0045-uid-platform-contract/README.md` |
| T-0046 | `.meta/archive/tasks/0046-anonymous-volume-contract/README.md` |
| T-0047 | `.meta/archive/tasks/0047-port-forwarding-coverage/README.md` |
| T-0048 | `.meta/archive/tasks/0048-feature-acquisition-trust-tests/README.md` |
| T-0049 | `.meta/archive/tasks/0049-integration-smoke-fidelity/README.md` |
| T-0050 | `.meta/archive/tasks/0050-cli-docker-contract-tests/README.md` |
| T-0051 | `.meta/archive/tasks/0051-container-env-contract/README.md` |
| T-0052 | `.meta/archive/tasks/0052-prune-brittle-tests/README.md` |
| T-0053 | `.meta/archive/tasks/0053-property-and-matrix-tests/README.md` |
| T-0054 | `.meta/archive/tasks/0054-macos-uid-remap/README.md` |
| T-0055 | `.meta/archive/tasks/0055-default-docker-limits/README.md` |
| T-0056 | `.meta/archive/tasks/0056-missing-container-env-error/README.md` |
| T-0057 | `.meta/archive/tasks/0057-profile-list/README.md` |
| T-0058 | `.meta/archive/tasks/0058-diagnose-the-build-issue-evidenced-by-the-latest/README.md` |
| T-0059 | `.meta/archive/tasks/0059-nc-half-close-design/README.md` |
| T-0060 | `.meta/archive/tasks/0060-port-forward-connector/README.md` |
| T-0061 | `.meta/archive/tasks/0061-diagnose-the-autotag-ci-failure-evidenced-by-logs/README.md` |
| T-0062 | `.meta/archive/tasks/0062-eliminate-the-fork-exec-race-in-temporary-executable/README.md` |
| T-0063 | `.meta/archive/tasks/0063-remove-duplicate-ci-from-the-autotag-to-release/README.md` |
| T-0064 | `.meta/archive/tasks/0064-remove-the-test-only-unused-import-and-make/README.md` |
| T-0065 | `.meta/archive/tasks/0065-remove-github-actions-node-js-20-deprecation-warnings/README.md` |
| T-0066 | `.meta/archive/tasks/0066-bump-the-project-patch-version-from-0-1/README.md` |
| T-0067 | `.meta/archive/tasks/0067-fix-the-skipped-create-release-job-while-preserving/README.md` |
| T-0068 | `.meta/archive/tasks/0068-bump-the-project-patch-version-from-0-1/README.md` |
| T-0069 | `.meta/archive/tasks/0069-build-devcontainer-specs-whose-oci-feature-archives-contain/README.md` |
| T-0070 | `.meta/archive/tasks/0070-custom-ca-registry-initiative/README.md` |
| T-0071 | `.meta/archive/tasks/0071-decide-the-production-custom-ca-configuration-and-security/README.md` |
| T-0072 | `.meta/archive/tasks/0072-implement-the-accepted-opt-in-custom-ca-capability/README.md` |
| T-0073 | `.meta/archive/tasks/0073-implement-and-ci-enable-an-ignored-end-to/README.md` |
| T-0074 | `.meta/archive/tasks/0074-bump-the-project-patch-version-from-0-1/README.md` |
| T-0076 | `.meta/archive/tasks/0076-diagnose-why-dcc-v0-1-rejects-the-latest/README.md` |
| T-0077 | `.meta/archive/tasks/0077-make-feature-contributed-customizations-dcc-state-apply-the/README.md` |
| T-0078 | `.meta/archive/tasks/0078-bump-the-project-patch-version-from-0-1/README.md` |
| T-0075 | `.meta/archive/tasks/0075-harden-forward-tests-later-bind-collision-releases-every/README.md` |
| T-0079 | `.meta/archive/tasks/0079-automatically-place-a-gitignore-inside-each-dcc-managed/README.md` |
| T-0080 | `.meta/archive/tasks/0080-replace-t-0075-s-still-racy-released-address/README.md` |
| T-0081 | `.meta/archive/tasks/0081-enforce-non-publishing-least-privilege-token-permissions-for/README.md` |
| T-0082 | `.meta/archive/tasks/0082-add-dcc-profile-bootstrap-to-exclusively-create-a/README.md` |
| T-0083 | `.meta/archive/tasks/0083-commit-the-pre-existing-removal-of-the-obsolete/README.md` |
| T-0084 | `.meta/archive/tasks/0084-profile-running-indicator-design/README.md` |
| T-0085 | `.meta/archive/tasks/0085-profile-running-indicator/README.md` |
| T-0086 | `.meta/archive/tasks/0086-default-feature-repository-design/README.md` |
| T-0087 | `.meta/archive/tasks/0087-default-feature-repository/README.md` |
| T-0088 | `.meta/archive/tasks/0088-built-in-default-feature-repository/README.md` |
| T-0089 | `.meta/archive/tasks/0089-bump-the-project-patch-version-from-0-1/README.md` |
| T-0090 | `.meta/archive/tasks/0090-make-profile-bootstrap-initialize-a-missing-devcontainer-directory/README.md` |
| T-0091 | `.meta/archive/tasks/0091-bump-the-patch-version-from-0-1-7/README.md` |
| T-0092 | `.meta/archive/tasks/0092-feature-validation/README.md` |
| T-0093 | `.meta/archive/tasks/0093-bump-the-patch-version-from-0-1-8/README.md` |
| T-0094 | `.meta/archive/tasks/0094-attach-port-collision/README.md` |
| T-0095 | `.meta/archive/tasks/0095-relay-placement/README.md` |
| T-0096 | `.meta/archive/tasks/0096-container-relay-design/README.md` |
| T-0097 | `.meta/tasks/0097-container-relay-implementation/README.md` |
| T-0098 | `.meta/archive/tasks/0098-update-the-tls-smoke-test-docker-stub-for/README.md` |
| T-0099 | `.meta/archive/tasks/0099-bump-the-package-version-from-0-2-0/README.md` |
| T-0100 | `.meta/archive/tasks/0100-set-up-this-amazon-linux-2023-host-to/README.md` |
| T-0101 | `.meta/archive/tasks/0101-run-the-ci-docker-smoke-tests-against-the/README.md` |
| T-0102 | `.meta/archive/tasks/0102-upgrade-docker-to-meet-the-engine-28-forwarding/README.md` |
| T-0103 | `.meta/archive/tasks/0103-evaluate-build-startup-and-connection-of-a-dcc/README.md` |
| T-0104 | `.meta/archive/tasks/0104-test-concurrent-codex-yolo-run-and-interactive-attach/README.md` |
| T-0105 | `.meta/archive/tasks/0105-measure-dcc-build-memory-with-the-codex-profile/README.md` |
| T-0106 | `.meta/archive/tasks/0106-build-workspace-test-profile-and-measure-memory-usage/README.md` |
| T-0107 | `.meta/archive/tasks/0107-audit-feature-durable-paths-for-image-owned-versus/README.md` |
| T-0108 | `.meta/tasks/0108-persistence-image-ownership/README.md` |
| T-0109 | `.meta/archive/tasks/0109-implement-and-verify-the-linux-dcc-persistence-contract/README.md` |
| T-0110 | `.meta/archive/tasks/0110-prepare-and-qualify-only-an-unpublished-local-codex/README.md` |
| T-0111 | `.meta/archive/tasks/0111-reproduce-cold-one-shot-startup-status-137-and/README.md` |
| T-0112 | `.meta/archive/tasks/0112-readiness-fifo-ownership/README.md` |
| T-0113 | `.meta/tasks/0113-fast-command-teardown-race/README.md` |
| T-0114 | `.meta/tasks/0114-stop-unstarted-container/README.md` |
| T-0115 | `.meta/tasks/0115-job-reattachment-ergonomics/README.md` |
| T-0116 | `.meta/archive/tasks/0116-diagnose-ci-timeout-waiting-for-the-readiness-regression/README.md` |
| T-0117 | `.meta/archive/tasks/0117-readiness-fixture-uid-portability/README.md` |
| T-0118 | `.meta/archive/tasks/0118-bump-dcc-from-0-3-0-to-0/README.md` |
| T-0119 | `.meta/archive/tasks/0119-diagnose-one-shot-drain-ci-failure-with-status/README.md` |
| T-0120 | `.meta/archive/tasks/0120-task-directory-migration/README.md` |

## Supporting artifacts

| Old path | Explicit owner | New path | Baseline SHA-256 |
| --- | --- | --- | --- |
| `.meta/decisions/0001-state-seeding-from-image.md` | T-0022 | `.meta/archive/tasks/0022-state-seeding/decision-0001-state-seeding-from-image.md` | `1c535055bf28e2d367970f0db3e3682b65b4d9ed93c1738312d7c43913085733` |
| `.meta/decisions/0002-update-remote-user-uid-in-build-stage.md` | T-0026 | `.meta/archive/tasks/0026-update-remote-user-uid/decision-0002-update-remote-user-uid-in-build-stage.md` | `8353d1cd1eaee01d38d08b5fab373b1007edb6cb9ccbf216d45c81b8b43578f4` |
| `.meta/decisions/0003-remove-image-fast-path.md` | T-0027 | `.meta/archive/tasks/0027-remove-image-fast-path/decision-0003-remove-image-fast-path.md` | `9b5056668f35eecbad199f52aafd52be64de3de4c68b45aa2f09394aa12b8aad` |
| `.meta/decisions/0004-embed-supervisor-in-image.md` | T-0028 | `.meta/archive/tasks/0028-supervisor-delivery-model/decision-0004-embed-supervisor-in-image.md` | `5d5cf8857b440ea71764f5375adb4e15b357c486e2e8e77b916450d8784c6a91` |
| `.meta/decisions/0005-container-env-substitution.md` | T-0051 | `.meta/archive/tasks/0051-container-env-contract/decision-0005-container-env-substitution.md` | `7e3baff472109c45d2b87f9ee63a63d70e8ab8cfe44a1f62da19d6f4dbec9620` |
| `.meta/decisions/0006-require-missing-container-env-default.md` | T-0056 | `.meta/archive/tasks/0056-missing-container-env-error/decision-0006-require-missing-container-env-default.md` | `5aef7941d7bd789163731fa7d2b56c6da7069e89be22fe502ec0f70e7fa4e6ff` |
| `.meta/decisions/0007-registry-scoped-custom-ca.md` | T-0071 | `.meta/archive/tasks/0071-decide-the-production-custom-ca-configuration-and-security/decision-0007-registry-scoped-custom-ca.md` | `d85c77e5d35b878c05b897c647a805c6c2b61e9478ff78fc5a39defda5c8135a` |
| `.meta/decisions/0008-relay-in-development-container.md` | T-0095 | `.meta/archive/tasks/0095-relay-placement/decision-0008-relay-in-development-container.md` | `b6d4fa0ac7e8a71eff8f23e1bc9b736b0d42c368a87314d90e0b03db7c5b6c4e` |
| `.meta/decisions/0009-warn-and-defer-running-config-changes.md` | T-0095 | `.meta/archive/tasks/0095-relay-placement/decision-0009-warn-and-defer-running-config-changes.md` | `8156ec7a35d85e29d1198da655f79c52336c4e68d7697611e4643c92df5bec97` |
| `.meta/decisions/0010-persistence-image-ownership.md` | T-0108 | `.meta/tasks/0108-persistence-image-ownership/decision-0010-persistence-image-ownership.md` | `fb1ce1fd295cfc9a2131ad39fe97997542f786e4985a286e09ec7a9e79ea80bf` |
| `.meta/incidents/0067-skipped-release-publication.md` | T-0067 | `.meta/archive/tasks/0067-fix-the-skipped-create-release-job-while-preserving/incident-skipped-release-publication.md` | `7173174c992174f554b051e8e0c3d921a092da59deb6ec37050bf4d3c3654484` |
| `.meta/quality/0004-dcc-rewrite-quality.md` | T-0004 | `.meta/archive/tasks/0004-dcc-rewrite/quality-dcc-rewrite-quality.md` | `9f456bee3214ee347829601673d5f417e95f7c4edf27612bdc3900df0ad7ae9f` |
| `.meta/quality/0026-update-remote-user-uid-quality.md` | T-0026 | `.meta/archive/tasks/0026-update-remote-user-uid/quality-update-remote-user-uid-quality.md` | `e199f590c92dfd69d1fed871163f3ec14ee357ced939cbf550fdc917e1e30caf` |
| `.meta/quality/0043-test-quality-corrections-quality.md` | T-0043 | `.meta/archive/tasks/0043-test-quality-corrections/quality-test-quality-corrections-quality.md` | `d5a221e58066bfaaf887d4551f58d88dd3552fed580e52746bdfe0b89b9d8194` |
| `.meta/quality/0054-macos-uid-remap-quality.md` | T-0054 | `.meta/archive/tasks/0054-macos-uid-remap/quality-macos-uid-remap-quality.md` | `1942f0d9d0c752280a02b34fe0905e91b5090c7d828fbbbab381dc558e138360` |
| `.meta/quality/0055-default-docker-limits-quality.md` | T-0055 | `.meta/archive/tasks/0055-default-docker-limits/quality-default-docker-limits-quality.md` | `d559497ee24f2d4473963d5819b061d6edf450e8c7dd0c2c5e85a4052f29999d` |
| `.meta/quality/0056-missing-container-env-error-quality.md` | T-0056 | `.meta/archive/tasks/0056-missing-container-env-error/quality-missing-container-env-error-quality.md` | `caffb997c2411b0eb4fc598dd4866e6a0368b8ab4d0a86506727af685cfc41ef` |
| `.meta/quality/0057-profile-list-quality.md` | T-0057 | `.meta/archive/tasks/0057-profile-list/quality-profile-list-quality.md` | `d36fd5a274d99e20bd346421a22234cee84dcb38d89f588906a8f67f4103fc88` |
| `.meta/quality/0060-port-forward-connector-quality.md` | T-0060 | `.meta/archive/tasks/0060-port-forward-connector/quality-port-forward-connector-quality.md` | `334031c91d57de9b914303104b086faff219c0832afba2e9abbb25142e6fee0c` |
| `.meta/quality/0067-release-publication-gate-quality.md` | T-0067 | `.meta/archive/tasks/0067-fix-the-skipped-create-release-job-while-preserving/quality-release-publication-gate-quality.md` | `7e7e7120584ce794211ea8784fd1e7daa3baf718e1c5f11afb4af22e8fdd77da` |
| `.meta/quality/0071-custom-ca-design-quality.md` | T-0071 | `.meta/archive/tasks/0071-decide-the-production-custom-ca-configuration-and-security/quality-custom-ca-design-quality.md` | `51d41cbe8871413eea461001992c823ad5b1e29caf4f7f51a8448e9c8c1b7e62` |
| `.meta/quality/0072-custom-ca-implementation-quality.md` | T-0072 | `.meta/archive/tasks/0072-implement-the-accepted-opt-in-custom-ca-capability/quality-custom-ca-implementation-quality.md` | `0ea3c51c8ed5f107910efe2f13350389afa04b987d356bdab130ff14bc8b2cf4` |
| `.meta/quality/0073-tls-oci-docker-smoke-quality.md` | T-0073 | `.meta/archive/tasks/0073-implement-and-ci-enable-an-ignored-end-to/quality-tls-oci-docker-smoke-quality.md` | `c238c04e0448d5ac4f2280ceb303b5f20ef0335178e81b50dfa2febfb1d2b629` |
| `.meta/quality/0087-default-feature-repository-quality.md` | T-0087 | `.meta/archive/tasks/0087-default-feature-repository/quality-default-feature-repository-quality.md` | `74c485587685c88097d13a42b8bb70e52c8809173f3047b3be624f38c96d34da` |
| `.meta/quality/0092-feature-validation.md` | T-0092 | `.meta/archive/tasks/0092-feature-validation/quality-feature-validation.md` | `0267379b8035d3d2b875286f7b9196cccaabd26e1f2f4cfeec301fb4bfac7c1f` |
| `.meta/quality/0096-container-relay-design.md` | T-0096 | `.meta/archive/tasks/0096-container-relay-design/quality-container-relay-design.md` | `0fc3e835f57801aec91857f09679775385b5414d0c41548011c02f957ed681a9` |
| `.meta/quality/0097-container-relay-implementation.md` | T-0097 | `.meta/tasks/0097-container-relay-implementation/quality-container-relay-implementation.md` | `916ac9220e6df92d4277ece202d227eaea2e5e77b9e42a68b2795ab03dbcc9dd` |
| `.meta/quality/0101-docker-smoke-main.md` | T-0101 | `.meta/archive/tasks/0101-run-the-ci-docker-smoke-tests-against-the/quality-docker-smoke-main.md` | `725c7dfea5419b6a790d617bbaaa8c3dc4795edbd01df8536eea8cfcb04e3526` |
| `.meta/quality/0102-docker-upgrade.md` | T-0102 | `.meta/archive/tasks/0102-upgrade-docker-to-meet-the-engine-28-forwarding/quality-docker-upgrade.md` | `68a85adb7d2d18886c19ef2fd9b618e9aa15993a43752465c9629da77e5fc15f` |
| `.meta/quality/0103-codex-profile.md` | T-0103 | `.meta/archive/tasks/0103-evaluate-build-startup-and-connection-of-a-dcc/quality-codex-profile.md` | `22827c3cae7e3ceddcef39cd7506a2261994539fefd09d1d0862223a6d6cbfad` |
| `.meta/quality/0104-concurrent-codex-attach.md` | T-0104 | `.meta/archive/tasks/0104-test-concurrent-codex-yolo-run-and-interactive-attach/quality-concurrent-codex-attach.md` | `943016948fa3ed4dc24039c105dd0ea0eb31593680289ccf91294017883de872` |
| `.meta/quality/0105-build-memory.md` | T-0105 | `.meta/archive/tasks/0105-measure-dcc-build-memory-with-the-codex-profile/quality-build-memory.md` | `94712b950698bb6c8fc894748fbd63c27ee132c4a5a8097e7a8d2a9bfadb768f` |
| `.meta/quality/0106-real-profile-memory.md` | T-0106 | `.meta/archive/tasks/0106-build-workspace-test-profile-and-measure-memory-usage/quality-real-profile-memory.md` | `8c4358b95dd6c9d8339466071458d81fe5dca7babd1974e85aa70e500eae1a4a` |
| `.meta/quality/0107-feature-state-audit.md` | T-0107 | `.meta/archive/tasks/0107-audit-feature-durable-paths-for-image-owned-versus/quality-feature-state-audit.md` | `1ceaea10c48dbd020a47ef57556bc743774347d665712f9d2965f12aedf9ea74` |
| `.meta/quality/0108-persistence.md` | T-0108 | `.meta/tasks/0108-persistence-image-ownership/quality-persistence.md` | `574642aee50142cbc4651e6a1bc7e38395f8c6e8b38c126a5436042035a26b91` |
| `.meta/quality/0111-startup-reproduction.md` | T-0111 | `.meta/archive/tasks/0111-reproduce-cold-one-shot-startup-status-137-and/quality-startup-reproduction.md` | `8ab10cdae23a2c75da0e85c7759e382b050a26b5d7128bfce301f3e452cb7b91` |
| `.meta/quality/0112-readiness-fifo-ownership.md` | T-0112 | `.meta/archive/tasks/0112-readiness-fifo-ownership/quality-readiness-fifo-ownership.md` | `40f74452d23b1452cf46732fe794ade5b9da03204bd740e766642bca9f123980` |
| `.meta/quality/0113-interactive-disconnects.md` | T-0113 | `.meta/tasks/0113-fast-command-teardown-race/quality-interactive-disconnects.md` | `d91c32063fc67f02950f4ce180989e310334ce279aacde8827eeae4dcc6e87ec` |
| `.meta/quality/0113-teardown-races.md` | T-0113 | `.meta/tasks/0113-fast-command-teardown-race/quality-teardown-races.md` | `c57d48c057d0e08db807114ee8f13163ed91afb3e087eeaaa825768f8faf20c8` |
| `.meta/quality/0116-readiness-ci-timeout.md` | T-0116 | `.meta/archive/tasks/0116-diagnose-ci-timeout-waiting-for-the-readiness-regression/quality-readiness-ci-timeout.md` | `491da4f901192fae7b429ea5e403fe15ae97b1afb8ed79e7b29cca09487bba28` |
| `.meta/quality/0117-readiness-fixture-uid-portability.md` | T-0117 | `.meta/archive/tasks/0117-readiness-fixture-uid-portability/quality-readiness-fixture-uid-portability.md` | `1cc280436da97a86d2ea59a6c719ec225006633301813fa0c36bed99a7c952e4` |
| `.meta/quality/0119-one-shot-registration-ci.md` | T-0119 | `.meta/archive/tasks/0119-diagnose-one-shot-drain-ci-failure-with-status/quality-one-shot-registration-ci.md` | `cd59d489bc350050bc640dd01a3815a2276280155ab34d559f0f0db0120e46e7` |
| `.meta/tasks/0004-dcc-rewrite-brief.md` | T-0004 | `.meta/archive/tasks/0004-dcc-rewrite/dcc-rewrite-brief.md` | `c4a01d0c93ed7ebb6fee3816f20d16f2a2a8401f04ab7c9482dc4ca3d7a3e40b` |
| `.meta/tasks/0004-dcc-rewrite-notes.md` | T-0004 | `.meta/archive/tasks/0004-dcc-rewrite/dcc-rewrite-notes.md` | `79822ae31f132b2c60f47b372c6ccc7fa1d33333d6caacddb10353a398e64615` |
| `.meta/tasks/0005-config-schema-brief.md` | T-0005 | `.meta/archive/tasks/0005-config-schema/config-schema-brief.md` | `41b36a5646edcbcc3550cb824cca19d3e0c4878dd4489d27b267babb4bee04a6` |
| `.meta/tasks/0006-state-cache-brief.md` | T-0006 | `.meta/archive/tasks/0006-state-cache/state-cache-brief.md` | `577f911693ba2ed8f0ed40027510d8ab3b7ab4e238ca99f1c9d6993bbf289f94` |
| `.meta/tasks/0007-feature-metadata-brief.md` | T-0007 | `.meta/archive/tasks/0007-feature-metadata/feature-metadata-brief.md` | `3ad65b5fed2936894f9f181d7a3fe528b54f6959894a9b5de1d57be687560053` |
| `.meta/tasks/0008-build-prep-brief.md` | T-0008 | `.meta/archive/tasks/0008-build-prep/build-prep-brief.md` | `b6e2dbd27aef6611941752e1f44a46b3539f4594ac7d00db215a5263453f394c` |
| `.meta/tasks/0009-runtime-lifecycle-brief.md` | T-0009 | `.meta/archive/tasks/0009-runtime-lifecycle/runtime-lifecycle-brief.md` | `dd2b483ede783de522c8a1081e5c0b093da48ad3471cf13efe0a8632df64290f` |
| `.meta/tasks/0010-final-validation-brief.md` | T-0010 | `.meta/archive/tasks/0010-final-validation/final-validation-brief.md` | `bd962e5a88f86b75da52228d13a9a034548fe2aa6f953cd46fcc02a4b16f7155` |
| `.meta/tasks/0021-state-path-guards-brief.md` | T-0021 | `.meta/archive/tasks/0021-state-path-guards/state-path-guards-brief.md` | `26dbc7cc5ce0a65002be7d60fe42357aaf4f556ecb0ac5c76f7945b5d23d6e4d` |
| `.meta/tasks/0022-state-seeding-brief.md` | T-0022 | `.meta/archive/tasks/0022-state-seeding/state-seeding-brief.md` | `066b7be639bee2176416b115951b04202198fd45f556b01f642e4c5c4e121cfd` |
| `.meta/tasks/0023-cache-runtime-exposure-brief.md` | T-0023 | `.meta/archive/tasks/0023-cache-runtime-exposure/cache-runtime-exposure-brief.md` | `97b21ca91ffeba6cbfc9d3d6193cfe01734c1fbd259bcbaa12c2a9bc88f06bc4` |
| `.meta/tasks/0024-r1-supervisor-design.md` | T-0024-R1 | `.meta/archive/tasks/0024-r1-r1-supervisor-design/r1-supervisor-design.md` | `e7d6ae2e02316c05ab877d2544752c7d08607aab1a6eada81229a93cb26f8636` |
| `.meta/tasks/0024-supervisor-lifecycle-ownership-brief.md` | T-0024 | `.meta/archive/tasks/0024-supervisor-lifecycle-ownership/supervisor-lifecycle-ownership-brief.md` | `a8cf15c0b8ae55c9a92ae87eb3698eeb559ce8ca155f4502cc2577bb3bc77e15` |
| `.meta/tasks/0025-r1-startup-handshake-design.md` | T-0025 | `.meta/archive/tasks/0025-supervisor-startup-handshake/r1-startup-handshake-design.md` | `8bcb2706d6cd6e972032ef8e2b64efb2482b778371e68b2b859d6644e98960e3` |
| `.meta/tasks/0025-supervisor-startup-handshake-brief.md` | T-0025 | `.meta/archive/tasks/0025-supervisor-startup-handshake/supervisor-startup-handshake-brief.md` | `21bbf050f9481608eda1e2525ff3f5606c2ea4c77eff1549798a03440076ea30` |
| `.meta/tasks/0026-r1-uid-remap-design.md` | T-0026 | `.meta/archive/tasks/0026-update-remote-user-uid/r1-uid-remap-design.md` | `c8514c6bcf4d532121e8710305b6ab9d7fa1ea9df902c2d71bbdb492e1bae098` |
| `.meta/tasks/0026-update-remote-user-uid-brief.md` | T-0026 | `.meta/archive/tasks/0026-update-remote-user-uid/update-remote-user-uid-brief.md` | `8185c1b2a4ba82038974c173a9e1bd3578bffb9fad98490868f86b867cd77dfe` |
| `.meta/tasks/0027-remove-image-fast-path-brief.md` | T-0027 | `.meta/archive/tasks/0027-remove-image-fast-path/remove-image-fast-path-brief.md` | `770b7425bdc279a3ef46622ebd45fd6e943911fda14e792646810de8dd30d8f0` |
| `.meta/tasks/0028-supervisor-delivery-model-brief.md` | T-0028 | `.meta/archive/tasks/0028-supervisor-delivery-model/supervisor-delivery-model-brief.md` | `249a44fa169a641f26281ca31bbf5614316ffaa1052f802ba325bc12accb9de9` |
| `.meta/tasks/0032-whether-to-bake-hooks-brief.md` | T-0032 | `.meta/archive/tasks/0032-whether-to-bake-hooks/whether-to-bake-hooks-brief.md` | `e9ac0dbf37bb9f26f8a7de0e962083d9d33a42ce51d791d9ab1567a2fe4bdd19` |
| `.meta/tasks/0043-test-quality-corrections-brief.md` | T-0043 | `.meta/archive/tasks/0043-test-quality-corrections/test-quality-corrections-brief.md` | `207adae444119324ef6b3e7c5ead6bdccfc7eebebfd5cd59e354b7f17e4e7924` |
| `.meta/tasks/0043-test-quality-corrections-notes.md` | T-0043 | `.meta/archive/tasks/0043-test-quality-corrections/test-quality-corrections-notes.md` | `79f60d9a1788fd283578753596dd6d104ce5ae314760814d6b2eb8fc45de2830` |
| `.meta/tasks/0044-feature-install-tracing-brief.md` | T-0044 | `.meta/archive/tasks/0044-feature-install-tracing/feature-install-tracing-brief.md` | `a45affb6247782b20a0fa926291b99f05d6cae77cb1eed285454a599ad80abf1` |
| `.meta/tasks/0045-uid-platform-contract-brief.md` | T-0045 | `.meta/archive/tasks/0045-uid-platform-contract/uid-platform-contract-brief.md` | `a7c4155d08b89eb93c181efdd676d9320bfaea0f9c42a69c04907e52edc37638` |
| `.meta/tasks/0046-anonymous-volume-contract-brief.md` | T-0046 | `.meta/archive/tasks/0046-anonymous-volume-contract/anonymous-volume-contract-brief.md` | `39e64d7652010485d5a4990f5eb3b34e567a0d9204c70364bd92371731573b52` |
| `.meta/tasks/0047-port-forwarding-coverage-brief.md` | T-0047 | `.meta/archive/tasks/0047-port-forwarding-coverage/port-forwarding-coverage-brief.md` | `5d33cb324da02c57562a8623e3630816423f8c84f29a19b5748dfc0a6d526f4e` |
| `.meta/tasks/0048-feature-acquisition-trust-tests-brief.md` | T-0048 | `.meta/archive/tasks/0048-feature-acquisition-trust-tests/feature-acquisition-trust-tests-brief.md` | `7590e5e5d91d5b5a3063c52650b0c8fd63ea568fe2afbf161f8d4dc5863670ab` |
| `.meta/tasks/0049-integration-smoke-fidelity-brief.md` | T-0049 | `.meta/archive/tasks/0049-integration-smoke-fidelity/integration-smoke-fidelity-brief.md` | `1e8b7b929baff748f42ce18e131d588402e32989a8ef8606ae6789538a865430` |
| `.meta/tasks/0050-cli-docker-contract-tests-brief.md` | T-0050 | `.meta/archive/tasks/0050-cli-docker-contract-tests/cli-docker-contract-tests-brief.md` | `e2346887396ce043bbf08b79e412b8c4fc0a706a1e2acc08cbc9bed1d21abb83` |
| `.meta/tasks/0051-container-env-contract-brief.md` | T-0051 | `.meta/archive/tasks/0051-container-env-contract/container-env-contract-brief.md` | `511607ee5566d379f3beb2be802952c2a109c7fec53b8e45293d4596ba24fd74` |
| `.meta/tasks/0052-prune-brittle-tests-brief.md` | T-0052 | `.meta/archive/tasks/0052-prune-brittle-tests/prune-brittle-tests-brief.md` | `e3fba19fa8df0a7fdcf0c3ce895cc59e66516d1f3c85fada74c5d99a829fa48a` |
| `.meta/tasks/0053-property-and-matrix-tests-brief.md` | T-0053 | `.meta/archive/tasks/0053-property-and-matrix-tests/property-and-matrix-tests-brief.md` | `d52d9f3e11d63dd1d385bc7ea2be0f9e985aa4089536dc22890de09d91973254` |
| `.meta/tasks/0054-macos-uid-remap-brief.md` | T-0054 | `.meta/archive/tasks/0054-macos-uid-remap/macos-uid-remap-brief.md` | `9c530c6bcc4ced12fd558b3e8e67af351a5c8000928eaa099c1eb3f86e956f15` |
| `.meta/tasks/0055-default-docker-limits-brief.md` | T-0055 | `.meta/archive/tasks/0055-default-docker-limits/default-docker-limits-brief.md` | `564ddda4af5f3f85fc54d74a22498b215b6f2419b3fae9e49d98367bc4c702ae` |
| `.meta/tasks/0056-missing-container-env-error-brief.md` | T-0056 | `.meta/archive/tasks/0056-missing-container-env-error/missing-container-env-error-brief.md` | `3d78c57eeaf3be9658c555415267bfb94137dcd1e0b0e5ef6c6f459671bcfbba` |
| `.meta/tasks/0057-profile-list-brief.md` | T-0057 | `.meta/archive/tasks/0057-profile-list/profile-list-brief.md` | `112c2d55ba449830f546dbf1274ffa54b004d752ae05c7db0ea3b8d8939a965d` |
| `.meta/tasks/0059-nc-half-close-design.md` | T-0059 | `.meta/archive/tasks/0059-nc-half-close-design/nc-half-close-design.md` | `ccd6f4cd44d4d0bf38e93d20a1fb81ecef1f58df6b686b856913670845973a3e` |
| `.meta/tasks/0060-port-forward-connector-brief.md` | T-0060 | `.meta/archive/tasks/0060-port-forward-connector/port-forward-connector-brief.md` | `9b51c7f8f8c40eb8d58479f76174ad52f33c9899c2a7dbe300baa6ab9feb58d2` |
| `.meta/tasks/0070-custom-ca-registry-initiative.md` | T-0070 | `.meta/archive/tasks/0070-custom-ca-registry-initiative/custom-ca-registry-initiative.md` | `5980ff83ba9aa490c3c2b4d19a244f0dd7bf16fe39a6b4ed5f2d3214e2e7358c` |
| `.meta/tasks/0084-profile-running-indicator-design.md` | T-0084 | `.meta/archive/tasks/0084-profile-running-indicator-design/profile-running-indicator-design.md` | `7ada70ee4c9a78cc94794a0c4099daa7c0bc4a51d4148d95ebd5117620088aa4` |
| `.meta/tasks/0085-profile-running-indicator-notes.md` | T-0085 | `.meta/archive/tasks/0085-profile-running-indicator/profile-running-indicator-notes.md` | `447df0a23d57a9640e45a2cd623c0cc8e7a00736b35b9c1b9b96886e39a2e7ac` |
| `.meta/tasks/0086-default-feature-repository-design.md` | T-0086 | `.meta/archive/tasks/0086-default-feature-repository-design/default-feature-repository-design.md` | `803182d2a5fcd1bd74fafa1036ea1d63bc0b18a8dfaf5e19a7eab14a027072fa` |
| `.meta/tasks/0087-default-feature-repository-notes.md` | T-0087 | `.meta/archive/tasks/0087-default-feature-repository/default-feature-repository-notes.md` | `a7459d7d0a9af9c16e8b24b01043971c72e9c5cf2e9416bec97fe2bbae170110` |
| `.meta/tasks/0088-built-in-default-feature-repository-notes.md` | T-0088 | `.meta/archive/tasks/0088-built-in-default-feature-repository/built-in-default-feature-repository-notes.md` | `fad0b93cd3937378737f8fb57fe3bb2554060da7db8b84b3e18a403f87c43f35` |
| `.meta/tasks/0092-feature-validation.md` | T-0092 | `.meta/archive/tasks/0092-feature-validation/feature-validation.md` | `c4d3f8529fea2f8711a1235a8b9342a8e888c7778fe97326eca5fe5ea0bfefc5` |
| `.meta/tasks/0094-attach-port-collision.md` | T-0094 | `.meta/archive/tasks/0094-attach-port-collision/attach-port-collision.md` | `ae5300c5341a567d13910c7736af6e45d869d3475afffb365175a4a47cb1dfa3` |
| `.meta/tasks/0095-relay-placement.md` | T-0095 | `.meta/archive/tasks/0095-relay-placement/relay-placement.md` | `b25d8eb4147d12833f318382afbfa807db28c7142f4b61ceab2b09966a102cf5` |
| `.meta/tasks/0096-container-relay-design.md` | T-0096 | `.meta/archive/tasks/0096-container-relay-design/container-relay-design.md` | `dc0dc475d153a92bc65d32ac7c0690d09f4a81945ddab9116e3e4088841c15b5` |
| `.meta/tasks/0097-container-relay-implementation.md` | T-0097 | `.meta/tasks/0097-container-relay-implementation/container-relay-implementation.md` | `48ce8fad0abc452690908d47ee67c5a507fb338a3b59765995962f02a6dbe6f9` |
| `.meta/tasks/0108-exclusion-alternatives.md` | T-0108 | `.meta/tasks/0108-persistence-image-ownership/exclusion-alternatives.md` | `19e1def0e0467a58844bef24ef5d72594fd27d1996703529c10536f2c4c6b1f8` |
| `.meta/tasks/0108-execution.md` | T-0108 | `.meta/tasks/0108-persistence-image-ownership/execution.md` | `7aa524a4396245929e997e89c7ef64652ef55ff8e56072ba6560b2061cecb8f9` |
| `.meta/tasks/0108-persistence-design-exploration.md` | T-0108 | `.meta/tasks/0108-persistence-image-ownership/persistence-design-exploration.md` | `4792d35357cd60f7c9750043ccad48af8cc939f4a10684ee41ede1e772feb70f` |
| `.meta/tasks/0108-persistence-image-ownership.md` | T-0108 | `.meta/tasks/0108-persistence-image-ownership/persistence-image-ownership.md` | `74c6bf07c3403a1e8574d113c8bbe3ad0f27401de2aae6df345f297694e5ecfd` |
| `.meta/tasks/0112-readiness-fifo-ownership.md` | T-0112 | `.meta/archive/tasks/0112-readiness-fifo-ownership/readiness-fifo-ownership.md` | `6048fdc5547ea3eecd7e2465511c79a7ffbd1a1cd9c92ed709041d305a7069ca` |
| `.meta/tasks/0113-fast-command-teardown-race.md` | T-0113 | `.meta/tasks/0113-fast-command-teardown-race/fast-command-teardown-race.md` | `c6cfabf0d3cd62e521b75adaf812c583ec996b4806a14afd4819a131ee87798d` |
| `.meta/tasks/0114-stop-unstarted-container.md` | T-0114 | `.meta/tasks/0114-stop-unstarted-container/stop-unstarted-container.md` | `c661190637949792af0b64e8b8e900633dc6875c0b42b16715da98c6bbd3feeb` |
| `.meta/tasks/0115-job-reattachment-ergonomics.md` | T-0115 | `.meta/tasks/0115-job-reattachment-ergonomics/job-reattachment-ergonomics.md` | `cc5d9144cfbc83836466cedb01672bbe97506acfe813e62f0a132de5f5e09dc7` |
| `.meta/tasks/0117-readiness-fixture-uid-portability.md` | T-0117 | `.meta/archive/tasks/0117-readiness-fixture-uid-portability/readiness-fixture-uid-portability.md` | `f2324dcea3f6bd42751e8b1729592a2757ad321d0636256d034ce9ce3b5c1ec8` |
| `.meta/tasks/0120-task-directory-migration.md` | T-0120 | `.meta/archive/tasks/0120-task-directory-migration/task-directory-migration.md` | `ea2607663165f666ac9ef0696d4607e06b74b79147e2a152e643f5f8321333cc` |
| `.meta/threat-models/0004-dcc-runtime.md` | T-0004 | `.meta/archive/tasks/0004-dcc-rewrite/threat-dcc-runtime.md` | `3941f74cb4b494d0bcd3aa262999e5bb8cbd322e5d79fe31a5e88dd78dcafdbf` |
| `.meta/threat-models/0062-ci-ref-and-fixture.md` | T-0062 | `.meta/archive/tasks/0062-eliminate-the-fork-exec-race-in-temporary-executable/threat-ci-ref-and-fixture.md` | `1440b1e146512b360136dc41c5cd308877356150a23917a440fa2fa0b77e1170` |
| `.meta/threat-models/0063-release-ci-reuse.md` | T-0063 | `.meta/archive/tasks/0063-remove-duplicate-ci-from-the-autotag-to-release/threat-release-ci-reuse.md` | `b8b15987ffeff1751b0898ce3238d3eb35c127a5c19bae681083c5600cf3130d` |
| `.meta/threat-models/0070-custom-ca-oci.md` | T-0070 | `.meta/archive/tasks/0070-custom-ca-registry-initiative/threat-custom-ca-oci.md` | `79b8999e9b8cb8f24f476eade92430ee36e4c89bd696437a6fc3c5460aebf358` |
| `.meta/threat-models/0096-container-relay-design.md` | T-0096 | `.meta/archive/tasks/0096-container-relay-design/threat-container-relay-design.md` | `5d1bb2d76c722a3c1cea5f3cf579b527ac3649e949c75ce5d9ceebaeeac9ce39` |

## Cutover and exceptions

The old catalog is replaced by discovery indexes; every row field moves into the corresponding task README. This mapping and the baseline Git revision provide provenance for catalog-only records without creating a second lifecycle owner. Current tasks are bundled first. T-0120 moves to the archive only after integration checks and independent recovery/review. The primary pointer remains T-0108; global Scheduling: Running and pause source None move to the project cursor.

Historical supporting content is preserved except navigation and explicit historical-provenance annotations. Historical approvals and incomplete checks are not revalidated or upgraded by this migration. Later host Docker authorizations do not lift T-0097’s task-specific no-launch boundary. Applicable requirements are reconciled into current project documentation before originating tasks are archived.

External reference inventory found one source comment in `src/features/context.rs`; only its documentation path changes. No product logic or executable command changes are in scope.
