#!/usr/bin/env python3
"""Prebuilt native builds (horizon#11859): which builds native-build.yml makes, named as the launcher names them.

    python3 scripts/native_build.py sets [--sets prebuilt.toml]          # one line per set: its mods
    python3 scripts/native_build.py plan --mods "zoom minimap@2.0.0" [--base latest|nightly-<date>-<short>]
                                         [--targets all|<triple>,...] --out plan/
    BASE=<40 hex> MODS=<json> MATRIX=<json> python3 scripts/native_build.py signable   # sign job: "key target" lines

`plan` resolves one set for one base: the Nightly (its tag for the launcher, its 40-hex commit for the key),
each mod against the published index (its newest version unless `@version` names one; refused when unknown,
revoked, or `broken_on` that commit), and each target's request key (horizon_release_sign::native_build:
the same text, so the launcher looks up what CI built). Targets already published are left out. It writes
plan/plan.json and downloads each mod's file into plan/hnmods/, checked against the index's SHA-256.

A run builds ONE set, and only a set prebuilt.toml lists (exactly its items): nobody can dispatch a signed build
of any other mix of registry mods. Every build job of a run belongs to that set, so a mod's build script, which
runs in that job, can only ever replace a build of its own set.

`signable` is what the sign job (which holds the key) signs: it reads only the plan job's outputs (job outputs,
which no later job can rewrite; never the `plan` artifact, which a build job can), checks again that the mods are a
prebuilt.toml set, recomputes every row's key from the base, the mods and the row's target, and prints
`<key> <target>` per row. The sign job derives every file name from those two.
"""
import argparse
import hashlib
import json
import os
import re
import sys
import tomllib
import urllib.error
import urllib.request
from pathlib import Path

INDEX = "https://horizon-rust.github.io/horizon-mods/index.json"
REGISTRY = "horizon-rust/horizon-mods"
HORIZON = "horizon-rust/horizon"
ROOT = Path(__file__).resolve().parents[1]

# The launcher's toolchain host for each target (Horizon's launcher/src/rust_toolchain.rs), the GitHub-hosted
# runner that builds it, and the launcher release asset that runs there.
TARGETS = {
    "x86_64-unknown-linux-gnu": ("ubuntu-22.04", "horizon-launcher-linux-x86_64.tar.gz"),
    "x86_64-pc-windows-gnu": ("windows-latest", "horizon-launcher-windows-x64.zip"),
    "aarch64-pc-windows-gnullvm": ("windows-11-arm", "horizon-launcher-windows-arm64.zip"),
    "aarch64-apple-darwin": ("macos-15", "horizon-launcher-macos-arm64.zip"),
    "x86_64-apple-darwin": ("macos-15-intel", "horizon-launcher-macos-x64.zip"),
}
MAX_MODS = 64
SETS = ROOT / "prebuilt.toml"
# horizon_release_sign::native_build's field rules: what the launcher accepts in a request.
ID = re.compile(r"[a-z0-9_-]+")
VERSION = re.compile(r"[!-~]+")
HEX40 = re.compile(r"[0-9a-f]{40}")
HEX64 = re.compile(r"[0-9a-f]{64}")


def request_text(target, base, mods):
    """horizon_release_sign::native_build::Request::text: mods are (id, version, sha256), sorted by id."""
    lines = ["horizon native build request 1", f"target {target}", f"base {base.lower()}"]
    lines += [f"mod {i} {v} {s.lower()}" for i, v, s in sorted(mods)]
    return "\n".join(lines) + "\n"


def request_key(target, base, mods):
    return hashlib.sha256(request_text(target, base, mods).encode()).hexdigest()


def release_tag(base):
    return f"native-builds-{base[:12]}"


def archive(key, target):
    return f"{key[:16]}-{target}.tar.gz"


def signature(key, target):
    return f"{key[:16]}-{target}.sig"


def get(url, token=True):
    headers = {"User-Agent": "horizon-mods native-build"}
    if token and os.environ.get("GITHUB_TOKEN") and url.startswith("https://api.github.com/"):
        headers["Authorization"] = f"Bearer {os.environ['GITHUB_TOKEN']}"
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=120) as r:
        return r.read()


def exists(url):
    try:
        with urllib.request.urlopen(urllib.request.Request(url, method="HEAD"), timeout=60):
            return True
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return False
        raise


def nightly(base):
    """(launcher tag, 40-hex commit) of the Nightly `base` names: `latest`, or `nightly-<date>-<short>`. A Nightly
    stays buildable after it is pruned: its permanent `nightly-source/<date>-<short>` tag keeps the commit."""
    refs = json.loads(get(f"https://api.github.com/repos/{HORIZON}/git/matching-refs/tags/nightly-source/"))
    names = {r["ref"].removeprefix("refs/tags/nightly-source/"): r["object"]["sha"] for r in refs}
    if base == "latest":
        if not names:
            sys.exit("no Nightly has a nightly-source/ tag")
        name = max(names)
    else:
        name = base.removeprefix("nightly-")
        if name not in names:
            sys.exit(f"{base}: no such Nightly (nightly-<date>-<short>)")
    commit = json.loads(get(f"https://api.github.com/repos/{HORIZON}/commits/nightly-source/{name}"))["sha"]
    return f"nightly-{name}", commit


def pick(index, wanted, commit):
    """The index version each `id[@version]` of `wanted` names, refusing what the launcher wouldn't build."""
    revoked = {r.get("sha256") for r in index.get("revoked", [])}
    projects = {p["id"]: p for p in index["projects"]}
    picked = []
    for item in wanted:
        mod_id, _, version = item.partition("@")
        project = projects.get(mod_id)
        if not project or project.get("kind") != "native-mod":
            raise ValueError(f"{mod_id}: no native mod of that id in the registry")
        versions = [v for v in project["versions"] if v["file"]["sha256"] not in revoked]
        found = next((v for v in versions if v["version"] == version), None) if version else (versions or [None])[0]
        if not found:
            raise ValueError(f"{item}: no such published, unrevoked version")
        if any(b.get("horizon") == commit for b in found.get("broken_on", [])):
            raise ValueError(f"{mod_id} {found['version']} doesn't build on {commit[:7]} (broken_on)")
        picked.append(found | {"id": mod_id})
    ids = [p["id"] for p in picked]
    if len(set(ids)) != len(ids):
        raise ValueError("a mod is listed twice")
    if not 1 <= len(picked) <= MAX_MODS:
        raise ValueError(f"a set has 1 to {MAX_MODS} mods")
    return picked


def listed(wanted, sets_path=SETS):
    """`wanted` (its `id[@version]` items) if it is exactly one set of prebuilt.toml, else ValueError."""
    if sorted(wanted) not in [sorted(line.split()) for line in sets(sets_path)]:
        raise ValueError(f"{' '.join(wanted) or '(nothing)'}: not a set of prebuilt.toml; only those are built")
    return wanted


def plan(mods, base, targets, out):
    wanted = listed(mods.split())
    tag, commit = nightly(base)
    index = json.loads(get(INDEX, token=False))
    picked = pick(index, wanted, commit)
    out.mkdir(parents=True, exist_ok=True)
    hnmods = out / "hnmods"
    hnmods.mkdir(exist_ok=True)
    for v in picked:
        data = get(v["file"]["url"], token=False)
        if hashlib.sha256(data).hexdigest() != v["file"]["sha256"]:
            sys.exit(f"{v['file']['url']}: its SHA-256 isn't the index's")
        (hnmods / f"{v['id']}-{v['version']}.hnmod").write_bytes(data)
    triples = list(TARGETS) if targets == "all" else targets.split(",")
    request = [(v["id"], v["version"], v["file"]["sha256"]) for v in picked]
    builds = []
    for target in triples:
        if target not in TARGETS:
            sys.exit(f"{target}: not a target the launcher builds")
        key = request_key(target, commit, request)
        url = f"https://github.com/{REGISTRY}/releases/download/{release_tag(commit)}/{signature(key, target)}"
        if exists(url):
            print(f"{target}: already published ({key[:16]})")
            continue
        runner, launcher = TARGETS[target]
        builds.append({"target": target, "key": key, "runner": runner, "launcher": launcher,
                       "archive": archive(key, target), "signature": signature(key, target)})
    result = {"tag": tag, "base": commit, "release": release_tag(commit),
              "mods": [{"id": i, "version": v, "sha256": s} for i, v, s in sorted(request)], "builds": builds}
    (out / "plan.json").write_text(json.dumps(result, indent=1) + "\n")
    print(json.dumps({k: result[k] for k in ("tag", "base", "mods")}), f"{len(builds)} to build")
    return result


def signable(base, mods, matrix, sets_path=SETS):
    """[(key, target)] the sign job may sign, from the plan job's outputs only (see the module's doc)."""
    if not isinstance(base, str) or not HEX40.fullmatch(base):
        raise ValueError(f"{base!r}: not a 40-hex commit")
    if not isinstance(mods, list) or not mods:
        raise ValueError("no mods")
    request = []
    for m in mods:
        i, v, s = (m.get(k) if isinstance(m, dict) else None for k in ("id", "version", "sha256"))
        if not (isinstance(i, str) and ID.fullmatch(i) and isinstance(v, str) and VERSION.fullmatch(v)
                and isinstance(s, str) and HEX64.fullmatch(s)):
            raise ValueError(f"{m!r}: not a mod (id, version, sha256)")
        request.append((i, v, s))
    ids = [i for i, _, _ in request]
    if len(set(ids)) != len(ids) or len(ids) > MAX_MODS:
        raise ValueError("a mod is listed twice, or too many mods")
    versions = dict((i, v) for i, v, _ in request)
    for line in sets(sets_path):
        items = [item.partition("@") for item in line.split()]
        if sorted(i for i, _, _ in items) == sorted(ids) and all(not v or versions[i] == v for i, _, v in items):
            break
    else:
        raise ValueError(f"{' '.join(sorted(ids))}: not a set of prebuilt.toml")
    if not isinstance(matrix, list):
        raise ValueError("the matrix is not a list")
    rows, seen = [], set()
    for row in matrix:
        target, key = (row.get(k) if isinstance(row, dict) else None for k in ("target", "key"))
        if target not in TARGETS or target in seen:
            raise ValueError(f"{target!r}: not a target, or twice")
        seen.add(target)
        if key != request_key(target, base, request):
            raise ValueError(f"{target}: the matrix's key {key!r} isn't this set's")
        rows.append((key, target))
    return rows


def sets(path):
    """Each set of prebuilt.toml as one line of space-separated `id[@version]`."""
    data = tomllib.loads(path.read_text())
    lines = []
    for s in data.get("set", []):
        mods = s.get("mods", [])
        if not mods or not all(isinstance(m, str) and m and " " not in m for m in mods):
            sys.exit(f"{path}: set {s.get('name', '?')!r} needs a list of mod ids")
        lines.append(" ".join(mods))
    return lines


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    s = sub.add_parser("sets")
    s.add_argument("--sets", type=Path, default=ROOT / "prebuilt.toml")
    p = sub.add_parser("plan")
    p.add_argument("--mods", required=True)
    p.add_argument("--base", default="latest")
    p.add_argument("--targets", default="all")
    p.add_argument("--out", type=Path, required=True)
    sub.add_parser("signable")
    args = parser.parse_args()
    if args.command == "sets":
        print("\n".join(sets(args.sets)))
        return
    if args.command == "signable":
        try:
            env = {k: json.loads(os.environ[k]) for k in ("MODS", "MATRIX")}
            rows = signable(os.environ["BASE"], env["MODS"], env["MATRIX"])
        except (KeyError, ValueError) as e:
            sys.exit(f"signable: {e}")
        print("\n".join(f"{key} {target}" for key, target in rows))
        return
    try:
        result = plan(args.mods, args.base, args.targets, args.out)
    except ValueError as e:
        sys.exit(str(e))
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as f:
            # The sign job reads these job outputs, never the plan artifact (a build job can replace that).
            f.write(f"matrix={json.dumps(result['builds'])}\n")
            f.write(f"any={'true' if result['builds'] else 'false'}\n")
            f.write(f"tag={result['tag']}\n")
            f.write(f"base={result['base']}\n")
            f.write(f"mods={json.dumps(result['mods'])}\n")


if __name__ == "__main__":
    main()
