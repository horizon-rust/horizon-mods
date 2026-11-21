#!/usr/bin/env python3
"""Which `staging` files cleanup.yml deletes. Staging holds each submitted file as <sha256>.<ext>; scripts/fetch.sh
takes a record's file from its own <id>-<version> release when that holds it (same name, uploaded, GitHub's digest
= the SHA-256), else from staging. A staging file goes once nothing can need it there:

  published: main's mods/ lists it and every record with its SHA-256 has its release's copy. After 1 day.
  abandoned: neither main nor an open pull request lists it. A retired .hnmod goes at once, others after 7 days.

Kept for good: a file that main's archive/ lists (frozen history, possibly staging's only copy), and one an open
pull request's mods/*.toml lists in a record without its release's copy (main's published records, which every
pull request carries, don't keep theirs).

  cleanup.py <now> <main> <assets> <releases> <heads>

<now> in Unix seconds; <main> a commit; <assets> has staging's files a line each, `<id>\t<name>\t<created_at>`;
<releases> the repository's releases a JSON line each, with tag_name, draft and assets (name, state, digest);
<heads> the commits open pull requests are at, a line each. Commits are read through git (GIT_DIR, else the
working directory's repository) as data, never checked out.
Prints `<id>\t<name>\t<why>` for each file to delete, oldest first, and a count of each kind to stderr.
"""
import datetime
import json
import pathlib
import re
import subprocess
import sys
import tomllib

DAY = 86400
PUBLISHED_DAYS = 1
ABANDONED_DAYS = 7
# The native-v1 format SDK launchers can't load: nothing stages new ones, so an unlisted one waits for nobody.
RETIRED = (".hnmod",)
ENTRY = re.compile(r"mods/[^/]+\.toml")
# Main's and archive/'s files keep every 64-digit hex run they hold as well as what their TOML gives: one that
# doesn't parse, or writes a hash in a way a pattern misses, still keeps what it names. So does a pull request's
# entry that doesn't parse.
HEX = re.compile(r"[0-9a-fA-F]{64}")


def git(*args, stdin=None):
    return subprocess.run(["git", *args], input=stdin, capture_output=True, check=True).stdout


def files(commit, folder, keep):
    """{path: text} of the regular files under `folder` at `commit` whose path `keep` accepts."""
    listed = []
    for item in filter(None, git("ls-tree", "-r", "-z", commit, "--", folder).split(b"\0")):
        meta, path = item.split(b"\t", 1)
        mode, kind, oid = meta.split(b" ")
        path = path.decode("utf-8", "replace")
        if kind == b"blob" and mode in (b"100644", b"100755") and keep(path):
            listed.append((path, oid))
    if not listed:
        return {}
    out, data = {}, git("cat-file", "--batch", stdin=b"".join(oid + b"\n" for _, oid in listed))
    for path, _ in listed:
        header, data = data.split(b"\n", 1)
        size = int(header.split(b" ")[2])
        out[path], data = data[:size].decode("utf-8", "replace"), data[size + 1:]
    return out


def parsed(text):
    try:
        return tomllib.loads(text)
    except tomllib.TOMLDecodeError:
        return None


def named(value):
    """Each `sha256` string anywhere in parsed TOML."""
    found, todo = set(), [value]
    while todo:
        value = todo.pop()
        if isinstance(value, dict):
            found.update(v.lower() for k, v in value.items() if k == "sha256" and isinstance(v, str))
            todo.extend(value.values())
        elif isinstance(value, list):
            todo.extend(value)
    return found


def hashes(text):
    """Every SHA-256 `text` names: each `sha256` string anywhere in its TOML, and each 64-digit hex run."""
    return {h.lower() for h in HEX.findall(text)} | named(parsed(text))


def entry_records(path, text):
    """Return ([(sha, tag, file)], protected_hashes) for one mods/*.toml entry.

    Malformed TOML, a non-list `versions` field, or any hash in the entry outside a complete
    `[[versions]]` record goes into `protected_hashes` so it stays kept independently of other entries.
    """
    entry = parsed(text)
    if entry is None:
        return [], hashes(text)
    raw_versions = entry.get("versions", [])
    if not isinstance(raw_versions, list):
        return [], hashes(text)
    recs, bad = [], set()
    stem = pathlib.PurePosixPath(path).stem
    for v in raw_versions:
        if isinstance(v, dict) and all(isinstance(v.get(k), str) for k in ("version", "file", "sha256")):
            recs.append((v["sha256"].lower(), f"{stem}-{v['version']}", v["file"]))
        else:
            bad |= named(v)
    covered = {sha for sha, _, _ in recs}
    bad |= hashes(text) - covered
    return recs, bad


def released(releases):
    """(tag, file name, digest) of every complete asset of a finished release other than staging."""
    out = set()
    for r in map(json.loads, filter(str.strip, releases.splitlines())):
        if r["tag_name"] != "staging" and not r.get("draft"):
            out.update((r["tag_name"], a["name"], a.get("digest")) for a in r["assets"] if a.get("state") == "uploaded")
    return out


def unreleased(sha, versions, have):
    """Whether some record with `sha`, or none at all, lacks its release's copy: fetch.sh would look in staging."""
    return not versions.get(sha) or any((tag, file, f"sha256:{sha}") not in have for tag, file in versions[sha])


def pinned(entries, have):
    """The SHA-256s an open pull request's mods/*.toml entries keep in staging."""
    out = set()
    for path, text in entries.items():
        recs, bad = entry_records(path, text)
        out |= bad
        for sha, tag, file in recs:
            if (tag, file, f"sha256:{sha}") not in have:
                out.add(sha)
    return out


def plan(now, main, assets, releases, heads):
    """[(id, name, why)] to delete, oldest first, and how many files each outcome had."""
    have = released(releases)
    entries = files(main, "mods", ENTRY.fullmatch)
    versions, protected = {}, set()
    for path, text in entries.items():
        recs, bad = entry_records(path, text)
        protected |= bad
        for sha, tag, file in recs:
            versions.setdefault(sha, []).append((tag, file))
    archived = set().union(*map(hashes, files(main, "archive", lambda _: True).values()))
    pending = set().union(*(pinned(files(head, "mods", ENTRY.fullmatch), have) for head in heads))
    out, counts = [], {}
    for created, id, name in sorted((c, i, n) for i, n, c in assets):
        sha = name.split(".", 1)[0]
        age = now - datetime.datetime.fromisoformat(created.replace("Z", "+00:00")).timestamp()
        if sha in pending:
            why = "kept: an open pull request lists it"
        elif sha in archived:
            why = "kept: archive/ lists it"
        elif sha in protected:
            why = "kept: published, entry could not be verified"
        elif sha in versions:
            if unreleased(sha, versions, have):
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
        print(__doc__.split("\n\n")[3], file=sys.stderr)
        return 2
    now, commit, assets, releases, heads = argv
    rows = [line.split("\t")[:3] for line in pathlib.Path(assets).read_text().splitlines() if line.strip()]
    out, counts = plan(int(now), commit, rows, pathlib.Path(releases).read_text(),
                       pathlib.Path(heads).read_text().split())
    for row in out:
        print("\t".join(row))
    for why, n in sorted(counts.items()):
        print(f"{n:5}  {why}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
