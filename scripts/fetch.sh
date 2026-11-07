#!/usr/bin/env bash
# Downloads every file the entries list into files/<sha256>, each checked against its SHA-256. Sources, in order:
# - the `registry-files` prerelease: a copy of every published file named <sha256>, kept by publish, so the
#   registry's own runs don't count as players' downloads (it reads it by URL, not through the API);
# - its own release (<id>-<version>) once published;
# - the `staging` prerelease, where the submission form uploads <sha256>.<extension>.
# A file already in files/ stays only if its SHA-256 is right.
# Usage: scripts/fetch.sh [checkout]. Needs `gh` (GH_TOKEN, GH_REPO), curl and cargo-horizon-mod.
# FILES_FROM=<dir> reads <dir>/<file> instead, for a local trial. FILES_MIRROR_UPLOAD=1 (publish, which can
# write releases) adds each file it had to take from elsewhere to registry-files.
set -euo pipefail
checkout="${1:-.}"
mirror=registry-files
mkdir -p files
sha_of() { sha256sum < "$1" | cut -d' ' -f1; }
cargo horizon-mod registry list "$checkout" | while read -r sha id version file; do
  [[ "$sha" =~ ^[0-9a-f]{64}$ ]] || { echo "::error::$id $version: $sha is not a SHA-256"; exit 1; }
  if [ -f "files/$sha" ] && [ ! -L "files/$sha" ] && [ "$(sha_of "files/$sha")" = "$sha" ]; then
    continue
  fi
  rm -rf "files/$sha"
  ext="${file##*.}"
  tmp="$(mktemp -d)"
  mkdir "$tmp/dl"  # gh downloads land here, apart from $tmp/file
  if [ -n "${FILES_FROM:-}" ]; then
    cp "$FILES_FROM/$file" "$tmp/file"
    from=folder
  elif [ -n "${GH_REPO:-}" ] \
    && curl -sfL --max-filesize 1073741824 -o "$tmp/file" "https://github.com/$GH_REPO/releases/download/$mirror/$sha" < /dev/null \
    && [ "$(sha_of "$tmp/file")" = "$sha" ]; then
    from=mirror
  elif gh release download "$id-$version" --pattern "$file" --dir "$tmp/dl" < /dev/null 2>/dev/null; then
    mv "$tmp/dl/$file" "$tmp/file"
    from="release $id-$version"
  elif gh release download staging --pattern "$sha.$ext" --dir "$tmp/dl" < /dev/null; then
    mv "$tmp/dl/$sha.$ext" "$tmp/file"
    from=staging
  else
    echo "::error::$id $version: $file is neither in release $id-$version nor in staging as $sha.$ext"
    exit 1
  fi
  if [ "$(sha_of "$tmp/file")" != "$sha" ]; then
    echo "::error::$id $version: $file from $from doesn't have the SHA-256 its entry lists ($sha)"
    exit 1
  fi
  if [ "${FILES_MIRROR_UPLOAD:-}" = 1 ] && [ "$from" != mirror ] && [ "$from" != folder ]; then
    gh release view "$mirror" > /dev/null 2>&1 < /dev/null \
      || gh release create "$mirror" --prerelease --latest=false --title "Registry files (for the registry's own runs)" \
        --notes "Every published mod file, named by its SHA-256, for publish and the checks to read without counting as a player's download (scripts/fetch.sh). Players download from each version's own release." < /dev/null
    cp "$tmp/file" "$tmp/dl/$sha"
    gh release upload "$mirror" "$tmp/dl/$sha" --clobber < /dev/null
  fi
  mv "$tmp/file" "files/$sha"
  rm -rf "$tmp"
done
