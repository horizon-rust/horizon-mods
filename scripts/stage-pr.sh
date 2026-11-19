#!/usr/bin/env bash
# For stage.yml: pull request $PR as GitHub has it now, read as data. When it is still open into main at the
# event's head $HEAD, its head and the main commit it leaves from are fetched into this checkout (objects only,
# nothing checked out), the pull request goes to $RUNNER_TEMP/pr.json, its description to <body>, the names of
# staging's complete files (uploaded, and their digest is their name's SHA-256) to <staged>, the number of all of
# staging's files to <staged>.count, and `base=<sha>` to $GITHUB_OUTPUT. Otherwise base is empty: a newer event's
# run stages it. With <base>, that base is required.
# Usage: scripts/stage-pr.sh <body> <staged> [<base>]
set -euo pipefail
body="$1" staged="$2" want="${3:-}"
moved() {
  echo "$1: nothing to stage for this event"
  echo "base=" >> "$GITHUB_OUTPUT"
  exit 0
}
gh api "repos/$GH_REPO/pulls/$PR" > "$RUNNER_TEMP/pr.json"
state="$(jq -r '"\(.state) \(.base.ref) \(.head.sha)"' "$RUNNER_TEMP/pr.json")"
[ "$state" = "open main $HEAD" ] || moved "#$PR is now \"$state\", not \"open main $HEAD\""
jq -r '.body // ""' "$RUNNER_TEMP/pr.json" > "$body"
base="$(gh api "repos/$GH_REPO/compare/main...$HEAD" --jq .merge_base_commit.sha)"
[[ "$base" =~ ^[0-9a-f]{40}$ ]]
[ -z "$want" ] || [ "$base" = "$want" ] || moved "#$PR now leaves main at $base, not $want"
git fetch -q --no-tags --depth=1 origin "$base" "+refs/pull/$PR/head:refs/stage/pr"
[ "$(git rev-parse refs/stage/pr)" = "$HEAD" ] || moved "#$PR's head is now $(git rev-parse refs/stage/pr)"
gh release view staging --json assets > "$RUNNER_TEMP/staging.json"
jq -r '.assets[] | select(.state == "uploaded" and .digest == "sha256:" + (.name | split(".")[0])) | .name' \
  "$RUNNER_TEMP/staging.json" > "$staged"
jq '.assets | length' "$RUNNER_TEMP/staging.json" > "$staged.count"
echo "base=$base" >> "$GITHUB_OUTPUT"
