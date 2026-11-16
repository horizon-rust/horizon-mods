#!/usr/bin/env python3
"""Frozen native-v1 history, never a second package validator or runtime index."""
import argparse
import html
import json
import pathlib
import shutil
import tomllib
import urllib.parse

ARCHIVE = pathlib.Path("archive/native-v1/mods")
RELEASES = "https://github.com/horizon-rust/horizon-mods/releases/download/"


def entries(root, folder):
    directory = root / folder
    if not directory.exists():
        return {}
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError(f"{directory}: expected a directory")
    result = {}
    for path in sorted(directory.iterdir()):
        if path.is_symlink() or not path.is_file() or path.suffix != ".toml":
            raise ValueError(f"{path}: expected a regular <id>.toml file")
        data = path.read_bytes()
        entry = tomllib.loads(data.decode())
        if entry["id"] != path.stem:
            raise ValueError(f"{path}: id does not match filename")
        result[path.stem] = (entry, data)
    return result


def review_base(base, head, out):
    """Allow only byte-preserving archival; keep normal registry review for live data."""
    old = entries(base, pathlib.Path("mods"))
    live = entries(head, pathlib.Path("mods"))
    before = entries(base, ARCHIVE)
    frozen = entries(head, ARCHIVE)
    for id, (_, data) in before.items():
        if id not in frozen or frozen[id][1] != data:
            raise ValueError(f"{id}: frozen historical metadata must not change or disappear")
    removed = []
    for id, (historic, data) in frozen.items():
        versions = historic.get("versions", [])
        kind = historic["kind"]
        extensions = {"native-mod": (".hnmod", ".hmod"), "modpack": (".hpack",)}.get(kind, ())
        if not versions or not extensions or any(not v["file"].endswith(extensions) for v in versions):
            raise ValueError(f"{id}: native-v1 history is only legacy natives and their modpacks")
        if id not in before:
            if id not in old or old[id][1] != data or id in live:
                raise ValueError(f"{id}: archive by moving the original entry unchanged, separately from reactivation")
            removed.append(id)
        if id in live:
            current = live[id][0]
            for field in ("kind", "owners", "owner_ids", "key"):
                if current.get(field) != historic.get(field):
                    raise ValueError(f"{id}: reactivation must preserve archived {field}")
            for v in current.get("versions", []):
                if any(v["version"] == h["version"] or v["sha256"] == h["sha256"] for h in versions):
                    raise ValueError(f"{id}: reactivation needs a new version and package, not a renamed historic bundle")
    for id, (entry, _) in live.items():
        if any(v["file"].endswith(".hnmod") for v in entry.get("versions", [])):
            raise ValueError(f"{id}: .hnmod is frozen history, not a live package")
    out.mkdir()
    shutil.copytree(base / "mods", out / "mods")
    if (base / "revoked.toml").exists():
        shutil.copyfile(base / "revoked.toml", out / "revoked.toml")
    for id in removed:
        (out / "mods" / f"{id}.toml").unlink()
        print(f"Archived {id}: original metadata and published assets retained; incompatible with the SDK runtime")


def history(root, out):
    frozen = entries(root, ARCHIVE)
    out.mkdir(parents=True, exist_ok=True)
    (out / "mods").mkdir(exist_ok=True)
    esc = html.escape
    parts = ["<!doctype html><meta charset='utf-8'><title>Horizon native-v1 history</title>",
             "<h1>Frozen native-v1 catalogue</h1>",
             "<p>These historical source bundles and dependent modpacks are incompatible with the .hmod SDK runtime. "
             "This is not a list of completed ports or an installable registry. Published release assets, signatures "
             "and original metadata are retained. No legacy runtime fallback is provided.</p>",
             "<p><a href='../../index.json'>Current registry</a> · <a href='catalogue.json'>Historical metadata (not a runtime index)</a></p>"]
    for id, (entry, data) in frozen.items():
        (out / "mods" / f"{id}.toml").write_bytes(data)
        parts.append(f"<section id='{esc(id)}'><h2>{esc(entry['name'])}</h2><p>{esc(id)} — {esc(entry['kind'])}</p>"
                     f"<p><a href='mods/{esc(id)}.toml'>Original entry, hashes and signatures</a></p>"
                     f"<pre>{esc(entry.get('description', ''))}</pre><ul>")
        for version in entry["versions"]:
            tag = urllib.parse.quote(f"{id}-{version['version']}", safe="")
            file = urllib.parse.quote(version['file'], safe="")
            parts.append(f"<li><a href='{esc(RELEASES + tag + '/' + file)}'>{esc(version['file'])}</a> "
                         f"SHA-256 <code>{esc(version['sha256'])}</code></li>")
        parts.append("</ul></section>")
    (out / "index.html").write_text("\n".join(parts) + "\n")
    (out / "catalogue.json").write_text(json.dumps({
        "archive": "native-v1", "installable": False,
        "reason": "Legacy .hnmod source bundles and dependent modpacks are incompatible with the .hmod SDK runtime",
        "projects": [entry for entry, _ in frozen.values()],
    }, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    review = commands.add_parser("review-base")
    for name in ("base", "head", "out"):
        review.add_argument(name, type=pathlib.Path)
    archive = commands.add_parser("history")
    for name in ("root", "out"):
        archive.add_argument(name, type=pathlib.Path)
    args = parser.parse_args()
    try:
        if args.command == "review-base":
            review_base(args.base, args.head, args.out)
        else:
            history(args.root, args.out)
    except (ValueError, KeyError, OSError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
