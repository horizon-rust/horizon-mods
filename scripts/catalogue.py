#!/usr/bin/env python3
"""Frozen pre-SDK histories, never a second package validator or runtime index.

Every read goes through git at an explicit commit: modes come from the tree, so no link (an entry's or any folder's
above it) is ever followed.
"""
import argparse
import html
import json
import pathlib
import re
import subprocess
import tomllib
import urllib.parse

ARCHIVE = "archive/native-v1/mods"
# Each frozen catalogue: its folder, the kinds it may hold with their package extensions, and what it is.
ARCHIVES = {
    "native-v1": (ARCHIVE, {"native-mod": (".hnmod", ".hmod"), "modpack": (".hpack",)},
                  "legacy natives and their modpacks",
                  "Legacy .hnmod source bundles and dependent modpacks are incompatible with the .hmod SDK runtime"),
    "api-0.1": ("archive/api-0.1/mods", {"shader": (".hshader",), "mod": (".hmod",)},
                "shader API 0.1 packs and pre-SDK content mods",
                "Shader packs for shader API 0.1 and content mods with pre-SDK manifests are incompatible with "
                "the shader API 0.2 and .hmod SDK runtime"),
}
# The only kind a reactivation may change: a legacy native may come back as a sandboxed content or wasm mod, which
# can do less than the native could. Every other archived kind comes back as itself.
PORTED_KINDS = {("native-mod", "mod")}
# The reviewed live-to-archive moves, one `<archive> <id>` per line, read from the trusted base only: no other
# pull request can take a live project out of the index by moving it.
MOVES = ".github/archive-moves.txt"
# The reviewed owner transfers of archived ids, one `<id> <old_owner_id> <new_owner_id> <new_login> <reason>` per line,
# also read from the trusted base only: a pull request can't hand an id to anyone by adding its own line.
TRANSFERS = ".github/owner-transfers.txt"
ID = re.compile(r"[a-z0-9_]{1,64}")
LOGIN = re.compile(r"[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})")
REGULAR = ("100644", "100755")
RELEASES = "https://github.com/horizon-rust/horizon-mods/releases/download/"


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True).stdout


def listing(repo, treeish):
    result = {}
    for record in git(repo, "ls-tree", "-z", treeish).split(b"\0"):
        if record:
            meta, name = record.split(b"\t", 1)
            mode, kind, obj = meta.decode().split()
            result[name.decode()] = (mode, kind, obj)
    return result


def folder(repo, sha, path):
    """The folder's entries at the commit, {} if it is absent; refused if it or a folder above it is not a folder."""
    current = sha
    for depth, part in enumerate(path.split("/")):
        found = listing(repo, current).get(part)
        if found is None:
            return {}
        mode, kind, current = found
        if mode != "040000" or kind != "tree":
            raise ValueError(f"{'/'.join(path.split('/')[:depth + 1])}: expected a folder")
    return listing(repo, current)


def regular_file(repo, sha, path):
    """The file's bytes at the commit, None if it is absent; refused unless it is a regular file."""
    parent, _, name = path.rpartition("/")
    found = folder(repo, sha, parent).get(name) if parent else listing(repo, sha).get(name)
    if found is None:
        return None
    mode, kind, obj = found
    if mode not in REGULAR or kind != "blob":
        raise ValueError(f"{path}: expected a regular file")
    return git(repo, "cat-file", "blob", obj)


def entries(repo, sha, path):
    result = {}
    for name, (mode, kind, obj) in sorted(folder(repo, sha, path).items()):
        id = name.removesuffix(".toml")
        if mode not in REGULAR or kind != "blob" or not name.endswith(".toml") or not ID.fullmatch(id):
            raise ValueError(f"{path}/{name}: expected a regular <id>.toml file")
        data = git(repo, "cat-file", "blob", obj)
        entry = tomllib.loads(data.decode())
        if entry.get("id") != id:
            raise ValueError(f"{path}/{name}: id does not match filename")
        result[id] = (entry, data)
    return result


def reviewed_moves(repo, sha):
    data = regular_file(repo, sha, MOVES) or b""
    moves = set()
    for line in data.decode().splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            name, id = line.split()
            moves.add((name, id))
    return moves


def reviewed_transfers(repo, sha):
    """{id: [(old_owner_id, new_owner_id, new_login), ...]} in file order."""
    data = regular_file(repo, sha, TRANSFERS) or b""
    transfers = {}
    for number, line in enumerate(data.decode().splitlines(), 1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(maxsplit=4)
        if (len(parts) != 5 or not ID.fullmatch(parts[0]) or not all(p.isascii() and p.isdigit() for p in parts[1:3])
                or not LOGIN.fullmatch(parts[3])):
            raise ValueError(f"{TRANSFERS}:{number}: expected <id> <old_owner_id> <new_owner_id> <new_login> <reason>")
        transfers.setdefault(parts[0], []).append((int(parts[1]), int(parts[2]), parts[3]))
    return transfers


def transferred_owners(id, historic, transfers):
    """The (owners, owner_ids) pairs an archived id may come back with after its reviewed transfers, in order: each
    transfer replaces one owner's login and id together. The archived pair itself is not in the list."""
    owners, ids = historic.get("owners", []), historic.get("owner_ids", [])
    states = []
    for old, new, login in transfers.get(id, []):
        if old not in ids or len(owners) != len(ids):
            raise ValueError(f"{id}: {TRANSFERS} hands over owner id {old}, which isn't one of its owner_ids")
        if new in ids:
            # Else a self-transfer, or a chain through a co-owner, would leave the outgoing account an owner.
            raise ValueError(f"{id}: {TRANSFERS} hands owner id {old} to {new}, which already is one of its owner_ids")
        at = ids.index(old)
        owners, ids = owners[:at] + [login] + owners[at + 1:], ids[:at] + [new] + ids[at + 1:]
        states.append((owners, ids))
    return states


def owned_by_author(historic, author_id):
    """The pull request's author (GitHub's own numeric account id, not a login or a form's claim) is a recorded owner.
    An entry without owner_ids never passes: a maintainer verifies its owner by hand."""
    return author_id in historic.get("owner_ids", [])


def review_archive(name, kinds, holds, old, live, before, frozen, moves, transfers, author_id, removed, unconfirmed):
    for id, (_, data) in before.items():
        if id not in frozen or frozen[id][1] != data:
            raise ValueError(f"{id}: frozen historical metadata must not change or disappear")
    for id, (historic, data) in frozen.items():
        versions = historic.get("versions", [])
        extensions = kinds.get(historic["kind"], ())
        if not versions or not extensions or any(not v["file"].endswith(extensions) for v in versions):
            raise ValueError(f"{id}: {name} history is only {holds}")
        if id not in before:
            if (name, id) not in moves:
                raise ValueError(f"{id}: moving it to {name} is not on the reviewed list {MOVES}")
            if id not in old or old[id][1] != data or id in live:
                raise ValueError(f"{id}: archive by moving the original entry unchanged, separately from reactivation")
            removed.append(id)
        if id in live:
            current = live[id][0]
            # After a reviewed transfer the entry may still hold the archived owners (until the pull request that
            # moves them merges) or the owners after any of its transfers, but only the newest owners count as the
            # author: the account it was handed from doesn't.
            states = transferred_owners(id, historic, transfers)
            owner = {"owner_ids": states[-1][1]} if states else historic
            # Not raised here: the check runs every other step first and fails on this last, so a maintainer
            # who confirms the owner by hand still has every other result for the exact head.
            if (id not in old or old[id][1] != live[id][1]) and not owned_by_author(owner, author_id):
                unconfirmed.append(f"{id}: this id is archived, and the pull request's author is not in its owner_ids; "
                                   "a maintainer must confirm the owner by hand")
            for field in ("kind", "owners", "owner_ids", "key"):
                if current.get(field) == historic.get(field):
                    continue
                if field in ("owners", "owner_ids") and (current.get("owners"), current.get("owner_ids")) in states:
                    continue
                if field == "kind" and (historic.get("kind"), current.get("kind")) in PORTED_KINDS:
                    continue
                raise ValueError(f"{id}: reactivation must preserve archived {field}")
            for v in current.get("versions", []):
                if any(v["version"] == h["version"] or v["sha256"] == h["sha256"] for h in versions):
                    raise ValueError(f"{id}: reactivation needs a new version and package, not a renamed historic bundle")


def review_base(base, base_sha, head, head_sha, out, author_id):
    """Allow only reviewed byte-preserving archival; keep normal registry review for live data.
    Returns the reactivations whose owner the pull request's author isn't (empty when every owner is confirmed)."""
    old = entries(base, base_sha, "mods")
    live = entries(head, head_sha, "mods")
    moves = reviewed_moves(base, base_sha)
    transfers = reviewed_transfers(base, base_sha)
    frozen ={name: entries(head, head_sha, path) for name, (path, *_) in ARCHIVES.items()}
    seen = set()
    for ids in frozen.values():
        if seen & ids.keys():
            raise ValueError(f"{min(seen & ids.keys())}: an id is frozen in one archive only")
        seen |= ids.keys()
    removed, unconfirmed = [], []
    for name, (path, kinds, holds, _) in ARCHIVES.items():
        review_archive(name, kinds, holds, old, live, entries(base, base_sha, path), frozen[name], moves, transfers,
                       author_id, removed, unconfirmed)
    (out / "mods").mkdir(parents=True)
    for id, (_, data) in old.items():
        if id not in removed:
            (out / "mods" / f"{id}.toml").write_bytes(data)
    revoked = regular_file(base, base_sha, "revoked.toml")
    if revoked is not None:
        (out / "revoked.toml").write_bytes(revoked)
    for id in removed:
        print(f"Archived {id}: original metadata and published assets retained; incompatible with the SDK runtime")
    return unconfirmed


def history(root, sha, out, name="native-v1"):
    path, _, _, reason = ARCHIVES[name]
    frozen = entries(root, sha, path)
    out.mkdir(parents=True, exist_ok=True)
    (out / "mods").mkdir(exist_ok=True)
    esc = html.escape
    parts = [f"<!doctype html><meta charset='utf-8'><title>Horizon {name} history</title>",
             f"<h1>Frozen {name} catalogue</h1>",
             f"<p>{esc(reason)}. "
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
        "archive": name, "installable": False,
        "reason": reason,
        "projects": [entry for entry, _ in frozen.values()],
    }, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    review = commands.add_parser("review-base")
    review.add_argument("base", type=pathlib.Path)
    review.add_argument("base_sha")
    review.add_argument("head", type=pathlib.Path)
    review.add_argument("head_sha")
    review.add_argument("out", type=pathlib.Path)
    review.add_argument("--author-id", type=int, required=True, help="the pull request author's numeric id, from GitHub")
    review.add_argument("--unconfirmed-owners", type=pathlib.Path, required=True,
                        help="file to write each reactivation whose owner is not confirmed to; empty if none")
    archive = commands.add_parser("history")
    archive.add_argument("root", type=pathlib.Path)
    archive.add_argument("sha")
    archive.add_argument("out", type=pathlib.Path)
    archive.add_argument("--archive", choices=ARCHIVES, default="native-v1")
    args = parser.parse_args()
    try:
        if args.command == "review-base":
            unconfirmed = review_base(args.base, args.base_sha, args.head, args.head_sha, args.out, args.author_id)
            args.unconfirmed_owners.write_text("".join(f"{line}\n" for line in unconfirmed))
        else:
            history(args.root, args.sha, args.out, args.archive)
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
