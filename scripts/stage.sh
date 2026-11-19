#!/usr/bin/env bash
# For a maintainer: puts the file of a hand-made pull request where its check looks for it, the
# `staging` prerelease as <sha256>.<extension>.
#
# The pull request's own check stages a file linked in its description (stage.yml); this is for
# a file linked elsewhere.
#
# Usage: scripts/stage.sh <pull request> <base sha> <head sha> \
#          https://github.com/<owner>/<repo>/releases/download/<tag>/<file>
#
# Run it from a checkout of main you trust, signed in to `gh` as a maintainer (GH_REPO, else
# horizon-rust/horizon-mods). <head sha> is the head you reviewed; <base sha> is where it leaves
# main, `git merge-base origin/main <head sha>`. The pull request is fetched as data: its
# mods/<id>.toml files are read at both commits as TOML (regular files only), never run, and its
# diff isn't read. The file is staged only when a [[versions]] record that is new in head names
# its file name and SHA-256. It is downloaded from GitHub (64 MiB at most), never opened or run.
# Needs git and python3 3.11+.
set -euo pipefail
repo="${GH_REPO:-horizon-rust/horizon-mods}"
pr="${1:-}" base="${2:-}" head="${3:-}" url="${4:-}"
if [[ $# -ne 4 || ! "$pr" =~ ^[0-9]+$ || ! "$base" =~ ^[0-9a-f]{40}$ || ! "$head" =~ ^[0-9a-f]{40}$ ]] \
  || [[ ! "$url" =~ ^https://github\.com/[A-Za-z0-9-]+/[A-Za-z0-9._-]+/releases/download/[^/?#[:space:]]+/[^/?#[:space:]]+$ ]]; then
  sed -n '8,9p' "$0" | sed 's/^# //' >&2
  exit 2
fi
root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
g() { git -C "$root" "$@"; }
tmp="$(mktemp -d)"
# Refs of this run only: another run at the same time has its own.
refs="refs/stage/$$-$RANDOM$RANDOM"
trap 'rm -rf "$tmp"; g update-ref -d "$refs/pr" 2>/dev/null; g update-ref -d "$refs/main" 2>/dev/null' EXIT

# The pull request as GitHub has it now: open, into main, at the head you reviewed.
still_reviewed() {
  gh pr view "$pr" --repo "$repo" --json state,baseRefName,headRefOid \
    --jq '"\(.state) \(.baseRefName) \(.headRefOid)"' > "$tmp/pr"
  if [ "$(cat "$tmp/pr")" != "OPEN main $head" ]; then
    echo "#$pr is now \"$(cat "$tmp/pr")\", not \"OPEN main $head\": review it again" >&2
    exit 1
  fi
}
still_reviewed
g fetch -q --no-tags "https://github.com/$repo" "+refs/pull/$pr/head:$refs/pr" "+refs/heads/main:$refs/main"
if [ "$(g rev-parse "$refs/pr")" != "$head" ]; then
  echo "#$pr's head is $(g rev-parse "$refs/pr"), not $head: review it again" >&2
  exit 1
fi
fork="$(g merge-base "$refs/main" "$head")"
if [ "$fork" != "$base" ]; then
  echo "$head leaves main at $fork, not $base" >&2
  exit 1
fi

# stage.py reads the entries and downloads the file; every value stays data (argv, files), never
# shell code.
mkdir "$tmp/files"
GIT_DIR="$(g rev-parse --absolute-git-dir)" python3 -I "$root/scripts/stage.py" download "$base" "$head" "$url" "$tmp/files"
name="$(ls "$tmp/files")"
gh release view staging --repo "$repo" --json assets --jq '.assets[].name' > "$tmp/staged"
if grep -Fxq "$name" "$tmp/staged"; then
  echo "staging already has $name"
  exit 0
fi
still_reviewed
gh release upload staging "$tmp/files/$name" --repo "$repo"
echo "Staged $name for #$pr: re-run its check."
