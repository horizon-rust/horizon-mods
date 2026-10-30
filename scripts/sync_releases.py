#!/usr/bin/env python3
"""Reads the GitHub releases of every project that names a repository (`repo` in mods/<id>.toml)
and keeps each new, checked one as a version in <checkout>/versions/<id>.json.

    scripts/sync_releases.py <checkout> [--only id,id] [--dry-run]

<checkout>/versions is the registry's `versions` branch (the workflow checks it out there); the index
build reads it. A release becomes a version only when the checks a submission passes pass on its asset
(`cargo horizon-mod registry release`: the kind's own check, the signature of a project with a key,
id and version in the file's manifest). One that fails is skipped and reported in an issue of the
registry (not with --dry-run, a pull request's trial); it is not read again until its asset changes.
Needs `gh` (GH_TOKEN, GH_REPO) and cargo-horizon-mod. REGISTRY_GAME is the Minecraft version for files
that name none. Nothing here runs a mod's code.
"""
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import tomllib
import urllib.request
from pathlib import Path

EXTENSIONS = {
    "mod": "hmod",
    "native-mod": "hnmod",
    "modpack": "hpack",
    "resourcepack": "zip",
    "shader": "hshader",
}
MAX_FILE = 64 << 20
# Dot-prefixed: a project id never is, so no project's cache file is this one.
REJECTED = ".rejected.json"
OWNERS = {}


def log(level, message):
    print(f"::{level}::{message}", flush=True)


def gh_json(path):
    out = subprocess.run(["gh", "api", "--paginate", "--slurp", path], capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError(out.stderr.strip() or f"gh api {path} failed")
    pages = json.loads(out.stdout)
    return [item for page in pages for item in (page if isinstance(page, list) else [page])]


def gh_text(path, ref):
    out = subprocess.run(
        ["gh", "api", "-H", "Accept: application/vnd.github.raw+json", f"{path}?ref={ref}"],
        capture_output=True,
        text=True,
    )
    if out.returncode != 0:
        if "Not Found" in out.stderr or "404" in out.stderr:
            return None
        raise RuntimeError(out.stderr.strip())
    return out.stdout


def download(url, to, limit):
    request = urllib.request.Request(url, headers={"User-Agent": "horizon-mods-registry"})
    sha = hashlib.sha256()
    size = 0
    with urllib.request.urlopen(request, timeout=300) as response, open(to, "wb") as out:
        while chunk := response.read(1 << 20):
            size += len(chunk)
            if size > limit:
                raise RuntimeError(f"over {limit >> 20} MiB")
            sha.update(chunk)
            out.write(chunk)
    return sha.hexdigest()


def tool(*args):
    out = subprocess.run(["cargo", "horizon-mod", *args], capture_output=True, text=True)
    if out.returncode != 0:
        raise RuntimeError((out.stdout + out.stderr).strip() or "the check failed")
    return out.stdout


def report(entry_id, tag, reason, quiet):
    log("warning", f"{entry_id} {tag}: not listed: {reason}")
    if quiet:
        return
    title = f"Release {tag} of {entry_id} was not listed"
    try:
        found = subprocess.run(
            ["gh", "issue", "list", "--state", "open", "--search", f'"{title}" in:title',
             "--json", "title", "--jq", ".[].title"],
            capture_output=True, text=True, check=True,
        ).stdout.splitlines()
        if title in found:
            return
        owners = ", ".join(f"@{o}" for o in OWNERS.get(entry_id, []))
        body = (
            f"The registry read the GitHub release `{tag}` of `{entry_id}` and did not list it. Nothing was published.\n\n"
            f"```\n{reason[:3000]}\n```\n\n"
            "It is read again when its asset changes; otherwise fix the file and publish a new release (a new tag).\n"
            f"Owners: {owners}"
        )
        subprocess.run(["gh", "issue", "create", "--title", title, "--body", body],
                       check=True, capture_output=True, text=True)
    except (subprocess.CalledProcessError, OSError) as e:
        log("warning", f"could not open an issue for {entry_id} {tag}: {e}")


def sync(checkout, entry, revoked, rejected, dry_run, game):
    entry_id, repo, kind = entry["id"], entry["repo"], entry["kind"]
    extension = EXTENSIONS.get(kind)
    if extension is None:
        log("warning", f"{entry_id}: kind {kind} has no release files yet")
        return False
    cache_path = checkout / "versions" / f"{entry_id}.json"
    cache = json.loads(cache_path.read_text()) if cache_path.exists() else {"page": {}, "versions": []}
    before = json.dumps(cache, sort_keys=True)
    legacy = {v["version"] for v in entry.get("versions", [])}
    # Releases older than the newest hosted version were never submitted: they stay unread.
    floor = max((v["published"] for v in entry.get("versions", [])), default="")
    listed_tags = {v["file"]["url"].split("/")[-2] for v in cache["versions"]}
    listed = legacy | {v["version"] for v in cache["versions"]}
    key = entry.get("key")

    releases = [r for r in gh_json(f"repos/{repo}/releases?per_page=100")
                if not r["draft"] and not r["prerelease"]]
    releases.sort(key=lambda r: r["published_at"] or "", reverse=True)
    for release in releases:
        tag = release["tag_name"]
        published = (release["published_at"] or "")[:10]
        if tag in listed_tags or published < floor:
            continue
        assets = [a for a in release["assets"] if a["name"].endswith(f".{extension}") and a["state"] == "uploaded"]
        if not assets:
            continue
        marker = f"{entry_id}/{tag}"
        asset = assets[0]
        stamp = f"{asset['id']}:{asset['updated_at']}:{asset['size']}"
        if rejected.get(marker) == stamp:
            continue
        try:
            if len(assets) > 1:
                raise RuntimeError(f"several .{extension} assets: keep one per release")
            if asset["size"] > MAX_FILE:
                raise RuntimeError(f"over {MAX_FILE >> 20} MiB")
            with tempfile.TemporaryDirectory() as tmp:
                path = Path(tmp) / asset["name"]
                sha = download(asset["browser_download_url"], path, MAX_FILE)
                digest = (asset.get("digest") or "").removeprefix("sha256:")
                if digest and digest != sha:
                    raise RuntimeError(f"GitHub's digest {digest} is not the downloaded file's {sha}")
                if any(r["id"] == entry_id and r.get("sha256") in (None, sha) for r in revoked):
                    log("notice", f"{entry_id} {tag}: revoked, not listed")
                    continue
                notes = Path(tmp) / "notes.md"
                notes.write_text(release["body"] or "")
                args = [str(checkout), entry_id, str(path), "--tag", tag, "--published", published,
                        "--game", game, "--notes", str(notes)]
                if key:
                    sig = next((a for a in release["assets"] if a["name"] == asset["name"] + ".minisig"), None)
                    if sig is None:
                        raise RuntimeError(f"the project has a key: add {asset['name']}.minisig to the release")
                    sig_path = Path(tmp) / "asset.minisig"
                    download(sig["browser_download_url"], sig_path, 1 << 16)
                    args += ["--signature", str(sig_path)]
                version = json.loads(tool("registry", "release", *args))
            if version["version"] in listed:
                # Hosted by the registry before: nothing to add. Two releases with one version: a mistake.
                if version["version"] not in legacy:
                    raise RuntimeError(f"version {version['version']} is already listed from another release")
                continue
            cache["versions"].append(version)
            listed.add(version["version"])
            log("notice", f"{entry_id} {version['version']}: listed from release {tag}")
            rejected.pop(marker, None)
        except Exception as e:  # noqa: BLE001 - every failure is a skipped release
            rejected[marker] = stamp
            # No issue for a trial run, or for a release of the day the registry's own last version
            # was published (the hosted versions' own repository releases may be older ones).
            report(entry_id, tag, str(e), dry_run or published == floor)
    cache["versions"].sort(key=lambda v: v["published"], reverse=True)

    # The page file of the repository, at its newest release.
    if releases:
        newest = releases[0]["tag_name"]
        try:
            text = gh_text(f"repos/{repo}/contents/horizon-registry.toml", newest)
            with tempfile.TemporaryDirectory() as tmp:
                page_file = Path(tmp) / "horizon-registry.toml"
                page_file.write_text(text or "")
                try:
                    cache["page"] = json.loads(tool("registry", "page", str(checkout), entry_id, str(page_file)))
                except RuntimeError as e:
                    if text:
                        log("warning", f"{entry_id}: horizon-registry.toml of {newest} not used: {e}")
        except RuntimeError as e:
            log("warning", f"{entry_id}: horizon-registry.toml: {e}")
    changed = json.dumps(cache, sort_keys=True) != before
    if changed:
        cache_path.write_text(json.dumps(cache, indent=1) + "\n")
    return changed


def main():
    args = sys.argv[1:]
    dry_run = "--dry-run" in args
    only = None
    if "--only" in args:
        only = {i for i in args[args.index("--only") + 1].split(",") if i}
    positional = [a for i, a in enumerate(args) if not a.startswith("--") and (i == 0 or args[i - 1] != "--only")]
    checkout = Path(positional[0] if positional else ".")
    game = os.environ.get("REGISTRY_GAME", "26.3")
    revoked_file = checkout / "revoked.toml"
    revoked = tomllib.loads(revoked_file.read_text()).get("revoked", []) if revoked_file.exists() else []
    (checkout / "versions").mkdir(exist_ok=True)
    rejected_path = checkout / "versions" / REJECTED
    rejected = json.loads(rejected_path.read_text()) if rejected_path.exists() else {}
    changed = 0
    for path in sorted((checkout / "mods").glob("*.toml")):
        entry = tomllib.loads(path.read_text())
        OWNERS[entry["id"]] = entry.get("owners", [])
        if "repo" not in entry or (only is not None and entry["id"] not in only):
            continue
        try:
            changed += sync(checkout, entry, revoked, rejected, dry_run, game)
        except Exception as e:  # noqa: BLE001 - one repository never stops the others
            log("warning", f"{entry['id']}: {entry['repo']}: {e}")
    rejected_path.write_text(json.dumps(rejected, indent=1, sort_keys=True) + "\n")
    print(f"{changed} project(s) changed")


if __name__ == "__main__":
    main()
