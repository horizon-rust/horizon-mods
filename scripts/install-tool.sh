#!/usr/bin/env bash
# Installs cargo-horizon-mod from the exact reviewed engine commit in .github/horizon-rev, never a floating branch.
# The pin is read beside this script, so a job takes it from the same trusted checkout as its scripts (check.yml:
# main's, never the pull request's). A file, not a repository variable: fork pull requests get no variables.
# Trusted jobs never restore executable or Cargo caches from mod-building jobs.
#
# checker.yml builds each pin once on main and publishes it as release checker@<rev>. That file is used only when
# GitHub's build attestation shows checker.yml on main built a file named for this exact pin (an older pin's binary
# re-uploaded under a new name fails); without one (a pin checker.yml hasn't finished, `--from-source`), this builds
# from source as before. `--prebuilt-only` fails instead of building: checker.yml's test for a usable build.
set -euo pipefail
pin="$(dirname "$0")/../.github/horizon-rev"
rev="$(tr -d '[:space:]' < "$pin")"
if [[ ! "$rev" =~ ^[0-9a-f]{40}$ ]]; then
  echo "::error::$pin must hold one exact 40-character lowercase commit"
  exit 1
fi
repo="${GH_REPO:-horizon-rust/horizon-mods}"
name="cargo-horizon-mod-$rev"
mode="${1:-}"
if [ "$mode" != --from-source ]; then
  dir="$(mktemp -d)"
  if gh release download "checker@$rev" --repo "$repo" --pattern "$name" --dir "$dir" 2>/dev/null \
    && gh attestation verify "$dir/$name" --repo "$repo" --signer-workflow "$repo/.github/workflows/checker.yml" \
      --source-ref refs/heads/main --deny-self-hosted-runners --format json > "$dir/attestation.json" \
    && jq -e --arg name "$name" 'any(.[].verificationResult.statement.subject[]; .name == $name)' \
      "$dir/attestation.json" > /dev/null; then
    mkdir -p "$HOME/.cargo/bin"
    install -m 755 "$dir/$name" "$HOME/.cargo/bin/cargo-horizon-mod"
    rm -rf "$dir"
    echo "cargo-horizon-mod $rev: checker.yml's attested build"
    exit 0
  fi
  rm -rf "$dir"
  if [ "$mode" = --prebuilt-only ]; then
    echo "no attested build of $rev from checker.yml"
    exit 1
  fi
  echo "no attested build of $rev from checker.yml: building it from source"
fi
cargo install --locked --force --git https://github.com/horizon-rust/horizon --rev "$rev" cargo-horizon-mod
