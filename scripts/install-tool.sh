#!/usr/bin/env bash
# Installs cargo-horizon-mod from Horizon at $HORIZON_REV (a commit; the repository
# variable HORIZON_REV, else main). The registry's checks are that tool's.
set -euo pipefail
rev="${HORIZON_REV:-main}"
if command -v cargo-horizon-mod >/dev/null && [ "$(cat ~/.cargo/horizon-rev 2>/dev/null)" = "$rev" ]; then
  exit 0
fi
cargo install --locked --git https://github.com/horizon-rust/horizon --rev "$rev" cargo-horizon-mod 2>/dev/null \
  || cargo install --locked --git https://github.com/horizon-rust/horizon --branch "$rev" cargo-horizon-mod
echo "$rev" > ~/.cargo/horizon-rev
