#!/usr/bin/env bash
# For a maintainer, while the submission form is down: puts the file of a hand-made pull request
# where its check looks for it, the `staging` prerelease as <sha256>.<extension>.
#
# Usage: scripts/stage.sh <pull request> <base sha> <head sha> \
#          https://github.com/<owner>/<repo>/releases/download/<tag>/<file>
#
# <head sha> is the pull request's head you reviewed, <base sha> the main commit it is compared
# with (`gh pr view <n> --json baseRefOid,headRefOid`). The mods/<id>.toml files it changes are read
# at both commits as TOML, never from the diff. The file is staged only when a [[versions]] record
# that is new in head names its SHA-256 and a `file` with its extension. It is downloaded (64 MiB at
# most), never opened or run. Needs git, python3 (3.11+) and `gh` signed in with write access to the
# registry (GH_REPO, else horizon-rust/horizon-mods); run it from a checkout of the registry.
set -euo pipefail
repo="${GH_REPO:-horizon-rust/horizon-mods}"
pr="${1:-}" base="${2:-}" head="${3:-}" url="${4:-}"
if [[ $# -ne 4 || ! "$pr" =~ ^[0-9]+$ || ! "$base" =~ ^[0-9a-f]{40}$ || ! "$head" =~ ^[0-9a-f]{40}$ ]] \
  || [[ ! "$url" =~ ^https://github\.com/[A-Za-z0-9-]+/[A-Za-z0-9._-]+/releases/download/[^/?#[:space:]]+/[^/?#[:space:]]+$ ]]; then
  sed -n '5,6p' "$0" | sed 's/^# //' >&2
  exit 2
fi
root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
g() { git -C "$root" "$@"; }
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"; g update-ref -d refs/stage/pr 2>/dev/null; g update-ref -d refs/stage/main 2>/dev/null' EXIT

# The pull request's head as GitHub has it now, and main: the commits must be those.
g fetch -q --no-tags "https://github.com/$repo" \
  "+refs/pull/$pr/head:refs/stage/pr" "+refs/heads/main:refs/stage/main"
if [ "$(g rev-parse refs/stage/pr)" != "$head" ]; then
  echo "#$pr's head is $(g rev-parse refs/stage/pr), not $head: review the new head first" >&2
  exit 1
fi
if ! g merge-base --is-ancestor "$base" refs/stage/main || ! g merge-base --is-ancestor "$base" "$head"; then
  echo "$base is not a main commit that $head is based on" >&2
  exit 1
fi

# Download, then match the bytes against the entries. Python reads the TOML; every value it is
# given or reads stays data (argv, files), never shell code.
GIT_DIR="$(g rev-parse --absolute-git-dir)" python3 -I - "$base" "$head" "$url" "$tmp" <<'PY'
import hashlib, os, pathlib, re, subprocess, sys, tomllib, urllib.error, urllib.request

base, head, url, tmp = sys.argv[1:]
MAX = 64 << 20
git = ["git", "--git-dir", os.environ["GIT_DIR"]]


class HttpsOnly(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if not newurl.startswith("https://"):
            raise urllib.error.HTTPError(newurl, code, "redirect away from https", headers, fp)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def versions(commit, path):
    shown = subprocess.run([*git, "show", f"{commit}:{path}"], capture_output=True)
    if shown.returncode != 0:
        return []  # a new entry
    try:
        entry = tomllib.loads(shown.stdout.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as e:
        sys.exit(f"{path} at {commit[:12]}: {e}")
    return [v for v in entry.get("versions", []) if isinstance(v, dict)]


changed = subprocess.run([*git, "diff", "--name-only", "-z", base, head, "--", "mods/"],
                         capture_output=True, check=True).stdout.decode("utf-8").split("\0")
added = []
for path in filter(None, changed):
    if not re.fullmatch(r"mods/[a-z0-9_]{1,64}\.toml", path):
        continue
    old = {v.get("sha256") for v in versions(base, path)}
    for v in versions(head, path):
        sha, file = v.get("sha256"), v.get("file")
        if isinstance(sha, str) and isinstance(file, str) and sha not in old:
            added.append((path, sha, file))
if not added:
    sys.exit("the pull request adds no [[versions]] record with a file and a sha256 to mods/")

digest, size = hashlib.sha256(), 0
opener = urllib.request.build_opener(HttpsOnly)
request = urllib.request.Request(url, headers={"User-Agent": "horizon-mods-stage"})
try:
    with opener.open(request, timeout=120) as response, open(pathlib.Path(tmp, "download"), "wb") as out:
        while chunk := response.read(1 << 16):
            size += len(chunk)
            if size > MAX:
                sys.exit("over 64 MiB, the registry's limit")
            digest.update(chunk)
            out.write(chunk)
except urllib.error.URLError as e:
    sys.exit(f"{url}: {e}")
sha = digest.hexdigest()
for path, want, file in added:
    ext = file.rsplit(".", 1)[-1] if "." in file else ""
    if want == sha and re.fullmatch(r"[a-z0-9]{1,16}", ext):
        pathlib.Path(tmp, "name").write_text(f"{sha}.{ext}")
        print(f"{path}: {file} has SHA-256 {sha}")
        break
else:
    listed = "\n  ".join(f"{p}: {f} {s}" for p, s, f in added)
    sys.exit(f"SHA-256 {sha} ({size} bytes) is not a new version's:\n  {listed}")
PY

name="$(cat "$tmp/name")"
gh release view staging --repo "$repo" --json assets --jq '.assets[].name' > "$tmp/staged"
if grep -Fxq "$name" "$tmp/staged"; then
  echo "staging already has $name"
  exit 0
fi
mv "$tmp/download" "$tmp/$name"
gh release upload staging "$tmp/$name" --repo "$repo"
echo "Staged $name for #$pr: re-run its check."
