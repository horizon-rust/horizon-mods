#!/usr/bin/env python3
"""Which `staging` files cleanup.yml deletes. Staging holds each submitted file as <sha256>.<ext>; a file goes
once nothing can need it there:

  published: main's mods/ lists it, and every [[versions]] record with its SHA-256 has its own <id>-<version>
             release holding that file (same name, uploaded, GitHub's digest = the SHA-256), so scripts/fetch.sh
             finds it there. Deleted after 1 day. A record whose release is missing or incomplete keeps it.
  abandoned: neither main nor an open pull request lists it. A retired .hnmod goes at once, others after 7 days.

A file that archive/ lists is kept for good (frozen history, possibly staging's only copy), as is one an open
pull request lists.

  cleanup.py <now> <checkout> <assets> <releases> <pending>

<now> in Unix seconds; <checkout> has main's mods/ and archive/; <assets> has staging's files a line each,
`<id>\t<name>\t<created_at>`; <releases> the repository's releases a JSON line each, with tag_name, draft and
assets (name, state, digest); <pending> the SHA-256s open pull requests add.
Prints `<id>\t<name>\t<why>` for each file to delete, oldest first, and a count of each kind to stderr.
"""
import datetime
import json
import pathlib
import re
import sys
import tomllib

DAY = 86400
PUBLISHED_DAYS = 1
ABANDONED_DAYS = 7
# The native-v1 format SDK launchers can't load: nothing stages new ones, so an unlisted one waits for nobody.
RETIRED = (".hnmod",)
SHA = re.compile(r'sha256 = "([0-9a-f]{64})"')


def listed(folder):
    """Every SHA-256 a file under `folder` names, as main's entries write it."""
    found = set()
    for path in sorted(pathlib.Path(folder).rglob("*")):
        if path.is_file():
            found.update(SHA.findall(path.read_text(encoding="utf-8", errors="replace")))
    return found


def records(mods):
    """SHA-256 -> [(tag, file)] of every [[versions]] record in mods/*.toml."""
    out = {}
    for path in sorted(pathlib.Path(mods).glob("*.toml")):
        entry = tomllib.loads(path.read_text(encoding="utf-8"))
        for v in entry.get("versions", []):
            if isinstance(v, dict) and all(isinstance(v.get(k), str) for k in ("version", "file", "sha256")):
                out.setdefault(v["sha256"], []).append((f"{path.stem}-{v['version']}", v["file"]))
    return out


def released(releases):
    """(tag, file name, digest) of every complete asset of a finished release other than staging."""
    out = set()
    for r in map(json.loads, filter(str.strip, releases.splitlines())):
        if r["tag_name"] != "staging" and not r.get("draft"):
            out.update((r["tag_name"], a["name"], a.get("digest")) for a in r["assets"] if a.get("state") == "uploaded")
    return out


def plan(now, checkout, assets, releases, pending):
    """[(id, name, why)] to delete, oldest first, and how many files each outcome had."""
    checkout = pathlib.Path(checkout)
    main, archived, versions = listed(checkout / "mods"), listed(checkout / "archive"), records(checkout / "mods")
    have = released(releases)
    out, counts = [], {}
    for created, id, name in sorted((c, i, n) for i, n, c in assets):
        sha = name.split(".", 1)[0]
        age = now - datetime.datetime.fromisoformat(created.replace("Z", "+00:00")).timestamp()
        if sha in pending:
            why = "kept: an open pull request lists it"
        elif sha in archived:
            why = "kept: archive/ lists it"
        elif sha in main:
            ours = versions.get(sha, [])
            if not ours or any((tag, file, f"sha256:{sha}") not in have for tag, file in ours):
                why = "kept: published, its release doesn't have it yet"
            elif age < PUBLISHED_DAYS * DAY:
                why = f"kept: published, under {PUBLISHED_DAYS} day"
            else:
                why = "deleted: published in its release"
        elif name.endswith(RETIRED):
            why = "deleted: abandoned, retired format"
        elif age < ABANDONED_DAYS * DAY:
            why = f"kept: abandoned, under {ABANDONED_DAYS} days"
        else:
            why = "deleted: abandoned"
        counts[why] = counts.get(why, 0) + 1
        if why.startswith("deleted"):
            out.append((id, name, why))
    return out, counts


def main(argv):
    if len(argv) != 5:
        print(__doc__.split("\n\n")[2], file=sys.stderr)
        return 2
    now, checkout, assets, releases, pending = argv
    rows = [line.split("\t")[:3] for line in pathlib.Path(assets).read_text().splitlines() if line.strip()]
    out, counts = plan(int(now), checkout, rows, pathlib.Path(releases).read_text(),
                       set(pathlib.Path(pending).read_text().split()))
    for row in out:
        print("\t".join(row))
    for why, n in sorted(counts.items()):
        print(f"{n:5}  {why}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
