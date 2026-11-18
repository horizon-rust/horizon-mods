#!/usr/bin/env bash
# For a maintainer, while the submission form is down: puts the file of a hand-made pull request
# where its check looks for it, the `staging` prerelease as <sha256>.<extension>.
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
  sed -n '5,6p' "$0" | sed 's/^# //' >&2
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

# Python reads the entries and downloads the file; every value stays data (argv, files), never
# shell code.
GIT_DIR="$(g rev-parse --absolute-git-dir)" python3 -I - "$base" "$head" "$url" "$tmp" <<'PY'
import hashlib, http.client, os, pathlib, re, subprocess, sys, tomllib, urllib.parse

base, head, url, tmp = sys.argv[1:]
MAX = 64 << 20
# github.com sends a release asset's download on to these.
HOSTS = {"github.com", "release-assets.githubusercontent.com", "objects.githubusercontent.com"}
git = ["git", "--git-dir", os.environ["GIT_DIR"]]


def shown(text):
    """A value from an entry, safe to print: no control characters."""
    return "".join(c if c.isprintable() else f"\\x{ord(c):02x}" for c in str(text))


def versions(commit, path):
    """The [[versions]] of `path` at `commit`; None when it isn't there. Only a regular file is
    read: a link's target or a folder isn't an entry."""
    tree = subprocess.run([*git, "ls-tree", "-z", commit, "--", path], capture_output=True, check=True).stdout
    if not tree:
        return None
    mode, kind, _ = tree.split(b" ", 2)
    if kind != b"blob" or mode not in (b"100644", b"100755"):
        sys.exit(f"{path} at {commit[:12]} is not a regular file ({mode.decode()} {kind.decode()})")
    blob = subprocess.run([*git, "show", f"{commit}:{path}"], capture_output=True, check=True).stdout
    try:
        entry = tomllib.loads(blob.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as e:
        sys.exit(f"{path} at {commit[:12]}: {shown(e)}")
    return [v for v in entry.get("versions", []) if isinstance(v, dict)]


def record(v):
    return tuple(v.get(k) for k in ("version", "file", "sha256"))


changed = subprocess.run([*git, "diff", "--name-only", "--no-renames", "-z", base, head, "--", "mods/"],
                         capture_output=True, check=True).stdout.decode("utf-8").split("\0")
added = []
for path in filter(None, changed):
    if not re.fullmatch(r"mods/[a-z0-9_]{1,64}\.toml", path):
        continue
    new = versions(head, path)
    if new is None:
        continue  # removed in head
    old = {record(v) for v in versions(base, path) or []}
    added += [(path, *record(v)) for v in new if record(v) not in old and all(isinstance(x, str) for x in record(v))]
if not added:
    sys.exit("the pull request adds no [[versions]] record with a version, a file and a sha256 to mods/")


def download(url, out):
    """`url` into `out`: redirects only to GitHub's asset hosts, a redirect's own body never read,
    at most MAX bytes. Its SHA-256."""
    for _ in range(5):
        parts = urllib.parse.urlsplit(url)
        if parts.scheme != "https" or parts.hostname not in HOSTS or parts.port not in (None, 443):
            sys.exit(f"refused: {shown(url)} is not on GitHub's asset hosts")
        conn = http.client.HTTPSConnection(parts.hostname, timeout=120)
        target = parts.path + (f"?{parts.query}" if parts.query else "")
        conn.request("GET", target, headers={"User-Agent": "horizon-mods-stage"})
        response = conn.getresponse()
        if response.status in (301, 302, 303, 307, 308):
            location = response.getheader("Location", "")
            conn.close()
            url = urllib.parse.urljoin(url, location)
            continue
        if response.status != 200:
            conn.close()
            sys.exit(f"{shown(url)}: HTTP {response.status}")
        digest, size = hashlib.sha256(), 0
        with open(out, "wb") as f:
            while chunk := response.read(1 << 16):
                size += len(chunk)
                if size > MAX:
                    conn.close()
                    sys.exit("over 64 MiB, the registry's limit")
                digest.update(chunk)
                f.write(chunk)
        conn.close()
        return digest.hexdigest(), size
    sys.exit("too many redirects")


name = urllib.parse.unquote(urllib.parse.urlsplit(url).path.rsplit("/", 1)[-1])
sha, size = download(url, pathlib.Path(tmp, "download"))
for path, version, file, want in added:
    ext = file.rsplit(".", 1)[-1] if "." in file else ""
    if (want, file) == (sha, name) and re.fullmatch(r"[a-z0-9]{1,16}", ext):
        pathlib.Path(tmp, "name").write_text(f"{sha}.{ext}")
        print(f"{path}: version {shown(version)}, {shown(file)}, SHA-256 {sha}")
        break
else:
    listed = "\n  ".join(f"{p}: version {shown(v)}, {shown(f)}, {shown(s)}" for p, v, f, s in added)
    sys.exit(f"{shown(name)} ({size} bytes, SHA-256 {sha}) is not a new version's file:\n  {listed}")
PY

name="$(cat "$tmp/name")"
gh release view staging --repo "$repo" --json assets --jq '.assets[].name' > "$tmp/staged"
if grep -Fxq "$name" "$tmp/staged"; then
  echo "staging already has $name"
  exit 0
fi
still_reviewed
mv "$tmp/download" "$tmp/$name"
gh release upload staging "$tmp/$name" --repo "$repo"
echo "Staged $name for #$pr: re-run its check."
