#!/usr/bin/env bash
# Download reviewed files into files/<sha256>. No Actions cache is trusted.
# registry-files contains content-addressed copies; original releases and staging
# remain the authoritative package sources. Every path is rehashed before use.
# FILES_FROM=<dir> reads local packages for a trial.
set -euo pipefail
checkout="${1:-.}"
mkdir -p files
cargo horizon-mod registry list "$checkout" | while read -r sha id version file; do
  [[ "$sha" =~ ^[0-9a-f]{64}$ ]] || exit 1
  [[ "$id" =~ ^[a-z0-9_]{1,64}$ ]] || exit 1
  [[ "$version" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$ ]] || exit 1
  [[ "$file" != .* && ${#file} -le 128 && "$file" =~ ^[A-Za-z0-9._+-]+$ ]] || exit 1
  if [ -f "files/$sha" ] && printf '%s  %s\n' "$sha" "files/$sha" | sha256sum --check --status; then
    continue
  fi
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  ext="${file##*.}"
  if [ -n "${FILES_FROM:-}" ]; then
    cp "$FILES_FROM/$file" "$tmp/package"
  elif curl --fail --silent --show-error --location \
      "https://github.com/${GH_REPO:-horizon-rust/horizon-mods}/releases/download/registry-files/$sha" \
      --output "$tmp/package"; then
    :
  elif gh release download "$id-$version" --pattern "$file" --dir "$tmp" 2>/dev/null; then
    mv "$tmp/$file" "$tmp/package"
  elif gh release download staging --pattern "$sha.$ext" --dir "$tmp"; then
    mv "$tmp/$sha.$ext" "$tmp/package"
  else
    echo "::error::$id $version: reviewed package is absent from its release and staging"
    exit 1
  fi
  printf '%s  %s\n' "$sha" "$tmp/package" | sha256sum --check --status || {
    echo "::error::$id $version: package SHA-256 mismatch"
    exit 1
  }
  mv "$tmp/package" "files/$sha"
  rm -rf "$tmp"
  trap - EXIT
done
