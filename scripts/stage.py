#!/usr/bin/env python3
"""Stages the files of a hand-made pull request: the `staging` prerelease holds each as <sha256>.<extension>,
where its check looks for it. Shared by scripts/stage.sh (a maintainer, any GitHub link) and
.github/workflows/stage.yml (automatic, from the links in the pull request's description).

The pull request is read as data through git at two explicit commits (GIT_DIR): the names it changes under mods/
and those mods/<id>.toml files, as TOML, regular files only. A file is staged only when a [[versions]] record that
is new in head names its file name and SHA-256. It is downloaded from GitHub (64 MiB at most), never opened or run.

  stage.py download <base> <head> <url> <dir>      the file at <url>, if it is a new record's
  stage.py links <base> <head> <body> <dir> [<staged>]
                                                   each new record's file, from its link in the text file <body>;
                                                   records whose <sha256>.<ext> is a line of <staged> are skipped
  stage.py check <dir>                             `cargo horizon-mod check` of each file `links` wrote
  stage.py verify <base> <head> <dir> <staged>     every file in <dir> is a new record's, rehashed, and every new
                                                   record's file is in <dir> or a line of <staged>
  stage.py ready <base> <head> <staged> <failures> whether a failed check is worth re-running: <failures> (its
                                                   failure annotations, a JSON string a line) name new records'
                                                   files as missing, and every new record's file is in <staged>

`download` and `links` write <dir>/<sha256>.<ext> and <dir>/../<dir name>.json (each new record: its entry's
path, id, kind and key, the record, and whether staging already had its file). `verify` prints the names in <dir>
that <staged> doesn't list. `ready` exits 1 when it isn't. Needs git and python3 3.11+.
"""
import hashlib
import http.client
import json
import os
import pathlib
import re
import subprocess
import sys
import tomllib
import urllib.parse

MAX = 64 << 20
# At most this many files a run: more is a maintainer's (scripts/stage.sh).
MAX_FILES = 3
# github.com sends a release asset's download on to these.
HOSTS = {"github.com", "release-assets.githubusercontent.com", "objects.githubusercontent.com"}
RELEASE = re.compile(r"https://github\.com/[A-Za-z0-9-]+/[A-Za-z0-9._-]+/releases/download/[^/?#\s]+/[^/?#\s]+")
# A link in Markdown text: up to the first space or character that closes Markdown or HTML around it.
URL = re.compile(r"https?://[^\s<>()\[\]\"'`]+", re.I)
ENTRY = re.compile(r"mods/[a-z0-9_]{1,64}\.toml")
EXT = re.compile(r"[a-z0-9]{1,16}")
# What scripts/fetch.sh reports when the check finds a file in neither its release nor staging.
ABSENT = ": reviewed package is absent from its release and staging"


class Refused(Exception):
    pass


def shown(text):
    """A value from an entry, safe to print: no control characters."""
    return "".join(c if c.isprintable() else f"\\x{ord(c):02x}" for c in str(text))


def git(*args):
    # GIT_DIR, else the repository of the working directory (stage.yml's checkout of main).
    return subprocess.run(["git", *args], capture_output=True, check=True).stdout


def entry(commit, path):
    """`path` at `commit` as TOML; None when it isn't there. Only a regular file is read: a link's target or a
    folder isn't an entry."""
    tree = git("ls-tree", "-z", commit, "--", path)
    if not tree:
        return None
    mode, kind, _ = tree.split(b" ", 2)
    if kind != b"blob" or mode not in (b"100644", b"100755"):
        raise Refused(f"{path} at {commit[:12]} is not a regular file ({mode.decode()} {kind.decode()})")
    try:
        return tomllib.loads(git("show", f"{commit}:{path}").decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as e:
        raise Refused(f"{path} at {commit[:12]}: {shown(e)}")


def records(path, found, commit):
    """(version, file, sha256, record) of each [[versions]] record; a record whose fields aren't strings stops the
    run, named."""
    out = []
    versions = found.get("versions", []) if found else []
    for n, v in enumerate(v for v in versions if isinstance(v, dict)):
        fields = tuple(v.get(k) for k in ("version", "file", "sha256"))
        bad = [k for k, x in zip(("version", "file", "sha256"), fields) if not isinstance(x, str)]
        if bad:
            raise Refused(f"{path} at {commit[:12]}: [[versions]] record {n + 1} (version {shown(v.get('version'))}): "
                          f"{', '.join(bad)} must be a string")
        out.append((*fields, v))
    return out


def added(base, head):
    """Each [[versions]] record new in head, in its mods/<id>.toml: a dict of what it is."""
    changed = git("diff", "--name-only", "--no-renames", "-z", base, head, "--", "mods/").decode("utf-8")
    out = []
    for path in filter(None, changed.split("\0")):
        if not ENTRY.fullmatch(path):
            continue
        new = entry(head, path)
        if new is None:
            continue  # removed in head
        old = {r[:3] for r in records(path, entry(base, path), base)}
        key = new.get("key")
        # Its format is the pinned checker's to judge (`check` passes it this key); any key needs signatures.
        if key is not None and not (isinstance(key, str) and key.strip()):
            raise Refused(f"{path} at {head[:12]}: key {shown(key)} is not a minisign public key")
        for version, file, sha, record in records(path, new, head):
            if (version, file, sha) in old:
                continue
            signature = record.get("signature")
            out.append({"path": path, "id": path[5:-5], "kind": new.get("kind"), "key": key, "version": version,
                        "file": file,
                        "sha256": sha, "signature": signature if isinstance(signature, str) else None,
                        "name": staged_name(sha, file)})
    return out


def staged_name(sha, file):
    """<sha256>.<ext>, its name in staging; None when the record can't have one."""
    ext = file.rsplit(".", 1)[-1] if "." in file else ""
    return f"{sha}.{ext}" if re.fullmatch(r"[0-9a-f]{64}", sha) and EXT.fullmatch(ext) else None


def download(url, out):
    """`url` into `out`: redirects only to GitHub's asset hosts, a redirect's own body never read, at most MAX
    bytes. Its SHA-256 and size."""
    for _ in range(5):
        parts = urllib.parse.urlsplit(url)
        if parts.scheme != "https" or parts.hostname not in HOSTS or parts.port not in (None, 443):
            raise Refused(f"{shown(url)} is not on GitHub's asset hosts")
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
            raise Refused(f"{shown(url)}: HTTP {response.status}")
        digest, size = hashlib.sha256(), 0
        with open(out, "wb") as f:
            while chunk := response.read(1 << 16):
                size += len(chunk)
                if size > MAX:
                    conn.close()
                    raise Refused("over 64 MiB, the registry's limit")
                digest.update(chunk)
                f.write(chunk)
        conn.close()
        return digest.hexdigest(), size
    raise Refused("too many redirects")


def last_segment(url):
    return urllib.parse.unquote(urllib.parse.urlsplit(url).path.rsplit("/", 1)[-1])


def link(body, file):
    """The one GitHub release download in `body` whose file name is `file`."""
    # A sentence may end right after a link.
    urls = (u.rstrip(".,;:!?") for u in URL.findall(body))
    named = list(dict.fromkeys(u for u in urls if last_segment(u) == file))
    github = [u for u in named if RELEASE.fullmatch(u)]
    if len(github) > 1:
        raise Refused(f"{len(github)} different links to {shown(file)}: link one")
    if github:
        return github[0]
    if named:
        raise Refused(f"{shown(named[0])} is not a GitHub release download "
                      "(https://github.com/<owner>/<repo>/releases/download/<tag>/<file>)")
    raise Refused(f"no link to {shown(file)} in the pull request's description")


def fetch(record, url, out):
    """Downloads `record`'s file from `url` into `out`/<sha256>.<ext>, refused unless its name and SHA-256 are the
    record's."""
    if record["name"] is None:
        raise Refused(f"{record['path']}: version {shown(record['version'])} has no plain <sha256> and extension")
    partial = out / f"{record['name']}.part"
    sha, size = download(url, partial)
    if (sha, last_segment(url)) != (record["sha256"], record["file"]):
        partial.unlink()
        raise Refused(f"{shown(last_segment(url))} ({size} bytes, SHA-256 {sha}) is not {record['path']}'s version "
                      f"{shown(record['version'])}: {shown(record['file'])}, SHA-256 {shown(record['sha256'])}")
    partial.rename(out / record["name"])
    print(f"{record['path']}: version {shown(record['version'])}, {shown(record['file'])}, SHA-256 {sha}")


def check(out):
    """The pinned checker on each file, as its entry's kind, with its key and signature when it has a key. It
    inspects the file and runs none of it."""
    failed = 0
    for r in json.loads((out.parent / f"{out.name}.json").read_text()):
        if r["staged"]:
            continue
        args = ["cargo", "horizon-mod", "check", str(out / r["name"]), "--kind", str(r["kind"])]
        if r["key"] is not None:
            if not r["signature"]:
                print(f"{r['path']}: version {shown(r['version'])} needs a signature: the entry has a key")
                failed = 1
                continue
            signature = out.parent / f"{r['name']}.minisig"
            signature.write_text(r["signature"])
            args += ["--key", r["key"], "--signature", str(signature)]
        print(f"{r['path']}: version {shown(r['version'])}", flush=True)
        failed |= subprocess.run(args).returncode != 0
    return failed


def plan(out, staged):
    (out.parent / f"{out.name}.json").write_text(json.dumps(staged, indent=1) + "\n")


def main(argv):
    mode, rest = (argv[0], argv[1:]) if argv else ("", [])
    sizes = {"download": (4,), "links": (4, 5), "check": (1,), "verify": (4,), "ready": (4,)}
    if len(rest) not in sizes.get(mode, ()):
        print(__doc__.split("\n\n")[2], file=sys.stderr)
        return 2
    if mode == "check":
        return check(pathlib.Path(rest[0]))
    base, head = rest[0], rest[1]
    new = added(base, head)
    if not new and mode not in ("verify", "ready"):
        if mode == "links":
            print("no new [[versions]] record: nothing to stage")
            plan(pathlib.Path(rest[3]), [])
            return 0
        raise Refused("the pull request adds no [[versions]] record with a version, a file and a sha256 to mods/")
    if mode == "download":
        url, out = rest[2], pathlib.Path(rest[3])
        name = last_segment(url)
        matching = [r for r in new if r["file"] == name]
        if not matching:
            listed = "\n  ".join(f"{r['path']}: version {shown(r['version'])}, {shown(r['file'])}" for r in new)
            raise Refused(f"{shown(name)} is not a new version's file:\n  {listed}")
        partial = out / "download"
        sha, size = download(url, partial)
        for r in matching:
            if r["sha256"] == sha and r["name"]:
                partial.rename(out / r["name"])
                plan(out, [{**r, "staged": False}])
                print(f"{r['path']}: version {shown(r['version'])}, {shown(r['file'])}, SHA-256 {sha}")
                return 0
        partial.unlink()
        listed = "\n  ".join(f"{r['path']}: version {shown(r['version'])}, {shown(r['file'])}, {shown(r['sha256'])}"
                             for r in new)
        raise Refused(f"{shown(name)} ({size} bytes, SHA-256 {sha}) is not a new version's file:\n  {listed}")
    if mode == "links":
        body, out = pathlib.Path(rest[2]).read_text(encoding="utf-8", errors="replace"), pathlib.Path(rest[3])
        have = set(pathlib.Path(rest[4]).read_text().split()) if len(rest) == 5 else set()
        for r in new:
            r["staged"] = r["name"] in have
            if r["staged"]:
                print(f"{r['path']}: version {shown(r['version'])}: staging already has {r['name']}")
        todo = [r for r in new if not r["staged"]]
        if len(todo) > MAX_FILES:
            raise Refused(f"{len(todo)} new files: at most {MAX_FILES} are staged automatically; "
                          "a maintainer stages them with scripts/stage.sh")
        for r in todo:
            fetch(r, link(body, r["file"]), out)
        plan(out, new)
        return 0
    if mode == "ready":
        return ready(new, lines(rest[2]), pathlib.Path(rest[3]).read_text().splitlines())
    out, have = pathlib.Path(rest[2]), lines(rest[3])
    names = {r["name"]: r for r in new if r["name"]}
    files = sorted(out.iterdir())
    if len(files) > MAX_FILES:
        raise Refused(f"{len(files)} files: at most {MAX_FILES}")
    for f in files:
        r = names.get(f.name)
        if r is None or not f.is_file() or f.is_symlink():
            raise Refused(f"{shown(f.name)} is not a new version's <sha256>.<ext>")
        if f.stat().st_size > MAX or hashlib.sha256(f.read_bytes()).hexdigest() != r["sha256"]:
            raise Refused(f"{f.name}: its SHA-256 is not {r['path']}'s version {shown(r['version'])}")
    missing = [r for r in new if r["name"] not in have and not (out / str(r["name"])).is_file()]
    if missing:
        raise Refused("neither staged nor downloaded: " + ", ".join(
            f"{r['path']} version {shown(r['version'])}" for r in missing))
    for f in files:
        if f.name not in have:
            print(f.name)
    return 0


def lines(path):
    return set(pathlib.Path(path).read_text().split())


def ready(new, have, failures):
    """0 when the check failed because new records' files were missing, and staging has all of them now."""
    absent = {json.loads(line) for line in failures if line.strip()}
    absent = {m.removesuffix(ABSENT) for m in absent if isinstance(m, str) and m.endswith(ABSENT)}
    if not absent:
        print("the check didn't fail on a missing file: not re-run")
        return 1
    ours = {f"{r['id']} {r['version']}" for r in new}
    if not absent <= ours:
        print(f"the check also misses files this pull request doesn't add ({shown(', '.join(sorted(absent - ours)))}): "
              "not re-run")
        return 1
    missing = [r for r in new if r["name"] not in have]
    if missing:
        print("staging doesn't have every new file yet (" + ", ".join(
            f"{r['path']} version {shown(r['version'])}" for r in missing) + "): not re-run")
        return 1
    print(f"the check missed {shown(', '.join(sorted(absent)))}, staged now")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except Refused as e:
        sys.exit(f"refused: {e}")
