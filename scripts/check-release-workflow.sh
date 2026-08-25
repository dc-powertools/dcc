#!/usr/bin/env bash
set -euo pipefail

release_workflow=.github/workflows/release.yml
autotag_workflow=.github/workflows/autotag.yml

job_block() {
  local workflow=$1
  local job=$2

  awk -v header="  ${job}:" '
    $0 == header { inside = 1 }
    inside && /^  [[:alnum:]_-]+:$/ && $0 != header { exit }
    inside { print }
  ' "$workflow"
}

require_job_line() {
  local workflow=$1
  local job=$2
  local expected=$3

  if ! job_block "$workflow" "$job" | grep -Fqx "$expected"; then
    printf 'workflow contract violation: %s job %s must contain: %s\n' \
      "$workflow" "$job" "$expected" >&2
    return 1
  fi
}

require_job_line "$release_workflow" ci \
  "    if: \${{ !inputs.ci_already_passed }}"
require_job_line "$release_workflow" build \
  '    needs: ci'
require_job_line "$release_workflow" build \
  "    if: \${{ always() && (inputs.ci_already_passed || needs.ci.result == 'success') }}"
require_job_line "$release_workflow" release \
  '    needs: build'
require_job_line "$release_workflow" release \
  "    if: \${{ !cancelled() && needs.build.result == 'success' }}"
require_job_line "$autotag_workflow" release \
  '      ci_already_passed: true'

printf 'release workflow contracts are valid\n'
