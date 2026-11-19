#!/usr/bin/env bash
# For stage.yml: re-runs pull request $PR's check of $HEAD once, when its first attempt failed because new files
# were missing that staging has now. One lookup, no waiting: a check still running is looked at again when it ends
# (stage.yml's workflow_run trigger). Needs scripts/stage-pr.sh's $RUNNER_TEMP/pr.json and <staged>, and $BASE.
# Usage: scripts/stage-rerun.sh <staged>
set -euo pipefail
staged="$1" tmp="$RUNNER_TEMP/rerun"
mkdir -p "$tmp"
# This pull request's check runs of $HEAD: those that name it, or, from a fork (its runs name no pull request),
# those from its head repository and branch. The newest counts.
gh api "repos/$GH_REPO/actions/workflows/check.yml/runs?head_sha=$HEAD&event=pull_request&per_page=30" \
  > "$tmp/runs.json"
run="$(jq -c --argjson n "$PR" --slurpfile pr "$RUNNER_TEMP/pr.json" '
  [.workflow_runs[] | select(if (.pull_requests | length) > 0 then any(.pull_requests[]; .number == $n)
    else .head_repository.full_name == $pr[0].head.repo.full_name and .head_branch == $pr[0].head.ref end)]
  | max_by(.id) // empty' "$tmp/runs.json")"
if [ -z "$run" ]; then
  echo "no check of #$PR at $HEAD yet: it will find the files in staging"
  exit 0
fi
read -r id status conclusion attempt prs <<< "$(jq -r \
  '"\(.id) \(.status) \(.conclusion) \(.run_attempt) \(.pull_requests | length)"' <<< "$run")"
if [ "$prs" -gt 1 ]; then
  echo "check run $id is for $prs pull requests: not re-run here, a maintainer re-runs it"
elif [ "$attempt" != 1 ]; then
  echo "check run $id is on attempt $attempt: a check is re-run once at most"
elif [ "$status" != completed ]; then
  echo "check run $id is $status: when it ends, stage looks at it again"
elif [ "$conclusion" != failure ]; then
  echo "check run $id ended $conclusion: nothing to re-run"
else
  gh api "repos/$GH_REPO/actions/runs/$id/attempts/1/jobs" --jq '.jobs[] | select(.conclusion == "failure") | .id' \
    > "$tmp/jobs"
  while read -r job; do
    gh api "repos/$GH_REPO/check-runs/$job/annotations?per_page=100" \
      --jq '.[] | select(.annotation_level == "failure") | .message | @json'
  done < "$tmp/jobs" > "$tmp/failures"
  python3 -I scripts/stage.py ready "$BASE" "$HEAD" "$staged" "$tmp/failures" || exit 0
  state="$(gh api "repos/$GH_REPO/pulls/$PR" --jq '"\(.state) \(.base.ref) \(.head.sha)"')"
  if [ "$state" != "open main $HEAD" ]; then
    echo "#$PR is now \"$state\": not re-run"
  elif gh api --silent -X POST "repos/$GH_REPO/actions/runs/$id/rerun-failed-jobs"; then
    echo "re-ran check run $id"
  else
    # Another run of this workflow may have re-run it just now.
    now="$(gh api "repos/$GH_REPO/actions/runs/$id" --jq '"\(.status) \(.run_attempt)"')"
    [ "$now" != "completed 1" ]
    echo "check run $id was re-run meanwhile ($now)"
  fi
fi
