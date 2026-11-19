#!/usr/bin/env bash
# Prints the id of the newest successful publish.yml run on main that this repository started itself (a push, the
# schedule or a dispatch), or nothing. Its `verified-files` artifact is then reused. A pull request from a fork
# whose branch is also called main, and which changes publish.yml, runs under that workflow too: its runs and
# artifacts must never be taken.
set -euo pipefail
repo="${GITHUB_REPOSITORY:?}"
gh api "repos/$repo/actions/workflows/publish.yml/runs?branch=main&status=success&per_page=50" \
  --jq "[.workflow_runs[] | select(.head_repository.full_name == \"$repo\"
          and (.event == \"push\" or .event == \"schedule\" or .event == \"workflow_dispatch\"))][0].id // empty"
