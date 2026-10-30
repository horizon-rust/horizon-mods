#!/usr/bin/env python3
"""Merges an owner update without a reviewer, when everything proves it is one.

The submission form (crates/horizon-mod-registry in horizon-rust/horizon) ends the body of an
owner's update with a trailer signed with REGISTRY_UPDATE_KEY:

    <!-- owner-update
    id: coords_hud
    version: 0.1.0
    sha256: <hex>
    file: coords_hud-0.1.0.hmod
    submitter: alice
    hmac: <hex HMAC-SHA256 of "owner-update-v1\\n<id>\\n<version>\\n<sha256>\\n<file>\\n<submitter>">
    -->

This script (run by .github/workflows/owner-update.yml, from main: never the pull request's copy) checks:
  a. the trailer's HMAC;
  b. the author is the form's bot account, and the diff is exactly mods/<id>.toml gaining one
     [[versions]] table that matches the trailer, every other field as it was;
  c. the submitter is in the entry's `owners` on main, and the id is not in revoked.toml;
  d. every workflow run on the head commit but this one succeeded, `check` among them.
Then it merges (with REGISTRY_MERGE_TOKEN, pinned to the verified commit). Anything wrong: it exits 1 and
the pull request waits for a reviewer as any other.

Environment: GH_TOKEN (read), MERGE_TOKEN, REPO, PR (number), BOT (the form's account),
UPDATE_KEY, SELF (this workflow's name, skipped while waiting).
"""
import hashlib
import hmac
import json
import os
import re
import subprocess
import sys
import time
import tomllib

TRAILER = re.compile(r"<!-- owner-update\r?\n(.*?)\r?\n-->", re.S)
FIELDS = ("id", "version", "sha256", "file", "submitter")
ID = re.compile(r"[a-z0-9_]{1,64}")
VERSION = re.compile(r"[A-Za-z0-9.+_-]+")
HEX64 = re.compile(r"[0-9a-f]{64}")
LOGIN = re.compile(r"[A-Za-z0-9-]{1,39}")


class Refused(Exception):
    pass


def trailer(body):
    """The trailer's fields, or None when the body has none (the last one counts)."""
    found = TRAILER.findall(body or "")
    if not found:
        return None
    fields = {}
    for line in found[-1].splitlines():
        key, sep, value = line.partition(": ")
        if not sep or key in fields:
            raise Refused(f"the trailer's line {line!r} is not `key: value`, or repeated")
        fields[key] = value.strip()
    if set(fields) != {*FIELDS, "hmac"}:
        raise Refused("the trailer has other fields than " + ", ".join((*FIELDS, "hmac")))
    return fields


def mac(key, fields):
    message = "\n".join(["owner-update-v1", *(fields[f] for f in FIELDS)])
    return hmac.new(key.encode(), message.encode(), hashlib.sha256).hexdigest()


def verify_trailer(key, fields):
    if not key:
        raise Refused("REGISTRY_UPDATE_KEY is not set")
    if not hmac.compare_digest(mac(key, fields), fields["hmac"].lower()):
        raise Refused("the trailer's HMAC is not valid: it was not made by the registry's form")
    if not (ID.fullmatch(fields["id"]) and VERSION.fullmatch(fields["version"])
            and HEX64.fullmatch(fields["sha256"]) and LOGIN.fullmatch(fields["submitter"])):
        raise Refused("the trailer's fields are malformed")


def check_diff(fields, files, base_text, head_text, main_text, revoked_text):
    """`files`: the pull request's changed files (name, status) against its merge base."""
    id_ = fields["id"]
    path = f"mods/{id_}.toml"
    if [(f["filename"], f["status"]) for f in files] != [(path, "modified")]:
        raise Refused(f"the diff is not exactly a modified {path}: {[f['filename'] for f in files]}")
    base, head, main = (tomllib.loads(t) for t in (base_text, head_text, main_text))
    if base != main:
        raise Refused(f"{path} changed on main since the pull request was opened")
    if head.get("id") != id_ or base.get("id") != id_:
        raise Refused("the entry's id is not the trailer's")
    if not any(o.lower() == fields["submitter"].lower() for o in base.get("owners", [])):
        raise Refused(f"{fields['submitter']} is not in {path}'s owners on main")
    revoked = [r for r in tomllib.loads(revoked_text).get("revoked", []) if r.get("id") == id_]
    if revoked:
        raise Refused(f"{id_} is in revoked.toml")
    rest = lambda e: {k: v for k, v in e.items() if k != "versions"}  # noqa: E731
    if rest(head) != rest(base):
        changed = sorted(k for k in {*rest(head), *rest(base)} if head.get(k) != base.get(k))
        raise Refused(f"it changes more than a version: {', '.join(changed)}")
    old, new = base.get("versions", []), head.get("versions", [])
    added = [v for v in new if v not in old]
    if len(new) != len(old) + 1 or len(added) != 1 or any(v not in new for v in old):
        raise Refused("it does not add exactly one version and keep the others")
    v = added[0]
    for field in ("version", "sha256", "file"):
        if v.get(field) != fields[field]:
            raise Refused(f"the new version's {field} is {v.get(field)!r}, not the signed {fields[field]!r}")
    if base.get("key") and not v.get("signature"):
        raise Refused("the entry has a key and the version has no signature")
    return base.get("kind")


def waiting(runs, own):
    """(finished, failure): the workflow runs but `own`, on the head commit."""
    others = [r for r in runs if r["name"] != own]
    if not any(r["name"] == "check" for r in others):
        return False, None
    for r in others:
        if r["status"] == "completed" and r["conclusion"] not in ("success", "skipped", "neutral"):
            return True, f"{r['name']} ended {r['conclusion']}"
    return all(r["status"] == "completed" for r in others), None


def gh(*args, token=None, raw=False):
    env = {**os.environ, "GH_TOKEN": token or os.environ["GH_TOKEN"]}
    out = subprocess.run(["gh", *args], env=env, capture_output=True, text=True)
    if out.returncode:
        raise Refused(f"gh {' '.join(args[:2])}: {out.stderr.strip()}")
    return out.stdout if raw else json.loads(out.stdout or "null")


def contents(repo, path, ref):
    return gh("api", "-H", "Accept: application/vnd.github.raw+json", f"repos/{repo}/contents/{path}?ref={ref}", raw=True)


def pull(repo, n):
    return gh("api", f"repos/{repo}/pulls/{n}")


def main():
    repo, n, bot = os.environ["REPO"], os.environ["PR"], os.environ["BOT"]
    pr = pull(repo, n)
    if pr["state"] != "open" or pr["base"]["ref"] != "main":
        print("not an open pull request to main")
        return 0
    fields = trailer(pr["body"])
    if fields is None:
        print("no owner-update trailer: a reviewer's")
        return 0
    verify_trailer(os.environ.get("UPDATE_KEY", ""), fields)
    if pr["user"]["login"] not in (bot, f"{bot}[bot]"):
        raise Refused(f"the author {pr['user']['login']} is not the form's account {bot}")
    sha = pr["head"]["sha"]
    compare = gh("api", f"repos/{repo}/compare/main...{sha}")
    if compare.get("total_commits", 0) < 1 or len(compare.get("files", [])) >= 300:
        raise Refused("the comparison is not a small one")
    path = f"mods/{fields['id']}.toml"
    kind = check_diff(
        fields,
        compare["files"],
        contents(repo, path, compare["merge_base_commit"]["sha"]),
        contents(repo, path, sha),
        contents(repo, path, "main"),
        contents(repo, "revoked.toml", "main"),
    )
    print(f"{fields['id']} {fields['version']} ({kind}) by {fields['submitter']}: the trailer and the diff hold")
    deadline = time.time() + 50 * 60
    while True:
        runs = gh("api", f"repos/{repo}/actions/runs?head_sha={sha}&per_page=100")["workflow_runs"]
        done, failure = waiting(runs, os.environ.get("SELF", "owner-update"))
        if failure:
            raise Refused(f"a check failed: {failure}")
        if done:
            break
        if time.time() > deadline:
            raise Refused("the checks did not finish in 50 minutes")
        time.sleep(30)
    if pull(repo, n)["head"]["sha"] != sha:
        raise Refused("the pull request changed while its checks ran")
    gh("pr", "merge", n, "--repo", repo, "--merge", "--match-head-commit", sha, token=os.environ["MERGE_TOKEN"])
    print(f"merged {sha}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Refused as e:
        print(f"::error::owner update not merged: {e}")
        sys.exit(1)
