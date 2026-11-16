#!/usr/bin/env bash
# HORIZON_REV must be the reviewed cutover engine commit, never a floating branch.
# Trusted jobs never restore executable or Cargo caches from mod-building jobs.
set -euo pipefail
rev="${HORIZON_REV:?Set HORIZON_REV to the reviewed .hmod SDK checker commit}"
if [[ ! "$rev" =~ ^[0-9a-f]{40}$ ]]; then
  echo "::error::HORIZON_REV must be an exact 40-character lowercase commit"
  exit 1
fi
cargo install --locked --force --git https://github.com/horizon-rust/horizon --rev "$rev" cargo-horizon-mod
