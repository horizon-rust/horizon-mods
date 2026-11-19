#!/usr/bin/env bash
# For stage.yml: pull request $PR as GitHub has it now, read as data. When it is still open into main at the
# event's head $HEAD, its head and the main commit it leaves from are fetched into this checkout (objects only,
# nothing checked out), its description goes to <body>, staging's file names to <staged>, and `base=<sha>` to
# $GITHUB_OUTPUT. Otherwise base is empty: a newer event's run stages it. With <base>, that base is required.
# Usage: scripts/stage-pr.sh <body> <staged> [<base>]
set -euo pipefail
body="$1" staged="$2" want="${3:-}"
moved() {
  echo "$1: nothing to stage for this event"
  echo "base=" >> "$GITHUB_OUTPUT"
  exit 0
}
state="$(gh api "repos/$GH_REPO/pulls/$PR" --jq '"\(.state) \(.base.ref) \(.head.sha)"')"
[ "$state" = "open main $HEAD" ] || moved "#$PR is now \"$state\", not \"open main $HEAD\""
gh api "repos/$GH_REPO/pulls/$PR" --jq '.body // ""' > "$body"
base="$(gh api "repos/$GH_REPO/compare/main...$HEAD" --jq .merge_base_commit.sha)"
[[ "$base" =~ ^[0-9a-f]{40}$ ]]
[ -z "$want" ] || [ "$base" = "$want" ] || moved "#$PR now leaves main at $base, not $want"
git fetch -q --no-tags --depth=1 origin "$base" "+refs/pull/$PR/head:refs/stage/pr"
[ "$(git rev-parse refs/stage/pr)" = "$HEAD" ] || moved "#$PR's head is now $(git rev-parse refs/stage/pr)"
gh release view staging --json assets --jq '.assets[].name' > "$staged"
echo "base=$base" >> "$GITHUB_OUTPUT"
