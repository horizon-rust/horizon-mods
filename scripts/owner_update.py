#!/usr/bin/env python3
"""Decides whether a pull request is an owner's update of a resource pack, shader pack or data pack, which merges
without a reviewer. Run by .github/workflows/owner-update.yml when `check` succeeds, from main: never the pull
request's copy. The pull request is read through the API and as git objects, as data.

It is one when all of these hold:
  a. it is open into main at the head whose `check` just succeeded, and the `check` check run on that head is a
     success;
  b. it changes exactly one file from where it leaves main: a modified mods/<id>.toml (live, a regular file), and
     that entry is the same on main now;
  c. the head's entry is that entry with one or more [[versions]] records added, every other value unchanged, and
     no new record is below one with the same `published` day;
  d. its kind is resourcepack, shader or datapack, and each new file has its kind's extension: extra files in them
     are accepted but never loaded (README, "Updates"), and every mod, native or not, keeps a reviewer;
  e. GitHub's numeric id of whoever opened it is in the entry's owner_ids (never a login or the description);
  f. the id is not in revoked.toml. When the entry has a key, each new version has a signature (the check verified
     it).
Then it writes `pr`, `sha`, `kind`, `id` and the blobs it read on main (`entry`: mods/<id>.toml, `revoked`:
revoked.toml) to $GITHUB_OUTPUT, so the merge can make sure main still has them. Anything else: the reason, and the
pull request waits for a reviewer as before. Its output holds the pull request's text, so the runner reads no
workflow command from it.

Environment: GH_TOKEN (read), GH_REPO, and the check's workflow run: HEAD_SHA, HEAD_REPO, HEAD_BRANCH, RUN_EVENT,
RUN_PATH. Needs git and python3 3.11+.
"""
import json
import os
import re
import secrets
import subprocess
import sys
import tomllib

KINDS = {"resourcepack": "a resource pack", "shader": "a shader pack", "datapack": "a data pack"}
# The index build that `check` runs refuses other extensions too; checked here as well since no reviewer follows.
EXTENSIONS = {"resourcepack": ".zip", "shader": ".hshader", "datapack": ".zip"}
ENTRY = re.compile(r"mods/([a-z0-9_]{1,64})\.toml")
SHA = re.compile(r"[0-9a-f]{40}")


class Refused(Exception):
    pass


def same(a, b):
    """Equal values of the same types all the way down: in Python 1 == 1.0 == True."""
    if type(a) is not type(b):
        return False
    if isinstance(a, dict):
        return a.keys() == b.keys() and all(same(a[k], b[k]) for k in a)
    if isinstance(a, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    return a == b


def parse(text, where):
    try:
        return tomllib.loads(text.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as e:
        raise Refused(f"{where} doesn't parse: {e}")


def eligible(changes, base_text, head_text, main_text, revoked_text, author_id):
    """`changes`: (status, path) of each file the pull request changes from where it leaves main. The texts are
    bytes: the entry where it leaves main, at its head and on main now, and revoked.toml on main. What it adds:
    (id, kind, [versions])."""
    if len(changes) != 1 or changes[0][0] != "M" or not ENTRY.fullmatch(changes[0][1]):
        listed = ", ".join(f"{s} {p}" for s, p in changes[:5])
        raise Refused(f"it changes more than one modified mods/<id>.toml: {listed}")
    path = changes[0][1]
    id_ = ENTRY.fullmatch(path)[1]
    if main_text != base_text:
        raise Refused(f"{path} changed on main since the pull request left it")
    base, head = parse(base_text, f"{path} on main"), parse(head_text, f"{path} at its head")
    if base.get("id") != id_:
        raise Refused(f"{path} on main has id {base.get('id')!r}")
    kind = base.get("kind")
    if kind not in KINDS:
        raise Refused(f"its kind is {kind!r}: only {', '.join(KINDS)} updates merge without a reviewer")
    rest = lambda e: {k: v for k, v in e.items() if k != "versions"}  # noqa: E731
    if not same(rest(head), rest(base)):
        changed = sorted(k for k in {*rest(head), *rest(base)} if not same(head.get(k), base.get(k)))
        raise Refused(f"it changes more than its versions: {', '.join(changed)}")
    old, new = base.get("versions"), head.get("versions")
    if not isinstance(old, list) or not isinstance(new, list):
        raise Refused("versions is not a list of records")
    added = [v for v in new if not any(same(v, o) for o in old)]
    kept = [v for v in new if any(same(v, o) for o in old)]
    if not added or not same(kept, old):
        raise Refused("it doesn't only add versions: every earlier record stays as it was, in its order")
    if any(not isinstance(v, dict) for v in added):
        raise Refused("a new version is not a record")
    # The index lists versions by `published` day, newest first, keeping a same-day tie in this order, and the
    # launcher takes the first as the newest: a new record below one of the same day would list that one as newest.
    days = []
    for v in new:
        day = v.get("published") if isinstance(v, dict) else None
        if isinstance(day, str) and day in days and any(v is a for a in added):
            raise Refused(f"version {v.get('version')!r} is below a version of the same day {day}: put it above that"
                          " one (two new versions of one day always wait for a reviewer)")
        days.append(day)
    for v in added:
        if not (isinstance(v.get("file"), str) and v["file"].endswith(EXTENSIONS[kind])):
            raise Refused(f"version {v.get('version')!r}'s file is not a {EXTENSIONS[kind]}")
    ids = base.get("owner_ids")
    if not isinstance(ids, list) or not ids:
        raise Refused(f"{path} has no owner_ids")
    if not (type(author_id) is int and any(type(i) is int and i == author_id for i in ids)):
        raise Refused(f"the account that opened it (id {author_id}) is not in {path}'s owner_ids")
    revoked = parse(revoked_text, "revoked.toml").get("revoked", [])
    if any(isinstance(r, dict) and r.get("id") == id_ for r in revoked):
        raise Refused(f"{id_} is in revoked.toml")
    if base.get("key") and any(not isinstance(v.get("signature"), str) for v in added):
        raise Refused("the entry has a key and a new version has no signature")
    return id_, kind, [str(v.get("version")) for v in added]


def pick(pulls, sha, repo, branch):
    """The one open pull request whose head is `sha` on `repo`'s `branch`. A fork's workflow run names no pull
    request, so it is found among the open ones."""
    found = [p for p in pulls if (p["sha"], p["repo"], p["ref"]) == (sha, repo, branch)]
    if len(found) != 1:
        raise Refused(f"{len(found)} open pull requests into main have head {sha} on {repo} {branch}")
    return found[0]["number"]


def check_passed(runs):
    """Whether the newest `check` check run GitHub Actions made on the head succeeded."""
    ours = [r for r in runs if r.get("name") == "check" and (r.get("app") or {}).get("slug") == "github-actions"]
    if not ours:
        return False
    newest = max(ours, key=lambda r: r["id"])
    return newest.get("status") == "completed" and newest.get("conclusion") == "success"


def gh(*args):
    out = subprocess.run(["gh", *args], capture_output=True, text=True)
    if out.returncode:
        raise Refused(f"gh {' '.join(args[:2])}: {out.stderr.strip()}")
    return out.stdout


def git(*args):
    return subprocess.run(["git", *args], capture_output=True, check=True).stdout


def blob(commit, path):
    """`path` at `commit`, bytes; refused unless it is a regular file."""
    tree = git("ls-tree", "-z", commit, "--", path)
    if not tree:
        raise Refused(f"{path} is not at {commit[:12]}")
    mode, kind, _ = tree.split(b" ", 2)
    if kind != b"blob" or mode not in (b"100644", b"100755"):
        raise Refused(f"{path} at {commit[:12]} is not a regular file")
    return git("show", f"{commit}:{path}")


def decide(env):
    if env["RUN_EVENT"] != "pull_request" or env["RUN_PATH"] != ".github/workflows/check.yml":
        raise Refused(f"not a pull request's check ({env['RUN_EVENT']}, {env['RUN_PATH']})")
    repo, sha = env["GH_REPO"], env["HEAD_SHA"]
    if not SHA.fullmatch(sha):
        raise Refused("no head commit")
    lines = gh("api", "--paginate", f"repos/{repo}/pulls?state=open&base=main&per_page=100", "--jq",
               ".[] | {number, sha: .head.sha, repo: .head.repo.full_name, ref: .head.ref}")
    n = pick([json.loads(line) for line in lines.splitlines() if line], sha, env["HEAD_REPO"], env["HEAD_BRANCH"])
    pr = json.loads(gh("api", f"repos/{repo}/pulls/{n}"))
    if (pr["state"], pr["base"]["ref"], pr["head"]["sha"]) != ("open", "main", sha):
        raise Refused(f"#{n} is not open into main at {sha}")
    if pr.get("draft"):
        raise Refused(f"#{n} is a draft")
    runs = json.loads(gh("api", f"repos/{repo}/commits/{sha}/check-runs?check_name=check&per_page=100"))
    if not check_passed(runs["check_runs"]):
        raise Refused(f"the required check `check` is not a success on {sha}")
    base = gh("api", f"repos/{repo}/compare/main...{sha}", "--jq", ".merge_base_commit.sha").strip()
    if not SHA.fullmatch(base):
        raise Refused("no merge base")
    git("fetch", "-q", "--no-tags", "--depth=1", "origin", base, f"+refs/pull/{n}/head:refs/owner/pr",
        "+refs/heads/main:refs/owner/main")
    if git("rev-parse", "refs/owner/pr").decode().strip() != sha:
        raise Refused(f"#{n}'s head moved on from {sha}")
    diff = git("diff", "--name-status", "--no-renames", "-z", base, sha).decode("utf-8").split("\0")
    changes = list(zip(diff[0::2], diff[1::2]))
    path = changes[0][1] if changes else ""
    main = git("rev-parse", "refs/owner/main").decode().strip()
    texts = [blob(c, path) for c in (base, sha, main)] if len(changes) == 1 and ENTRY.fullmatch(path) else [b""] * 3
    id_, kind, versions = eligible(changes, *texts, blob(main, "revoked.toml"), pr["user"]["id"])
    print(f"#{n}: {id_} {', '.join(versions)}, an owner's update of {KINDS[kind]} (opened by id {pr['user']['id']})")
    blobs = [git("rev-parse", f"{main}:{p}").decode().strip() for p in (path, "revoked.toml")]
    return n, kind, id_, *blobs


def main():
    # File names, TOML keys and versions are the pull request's: a line of them must not be a workflow command
    # (::set-output:: would set `pr`). The token is never printed, so commands stay off for the whole step.
    print(f"::stop-commands::{secrets.token_hex(16)}", flush=True)
    summary = os.environ.get("GITHUB_STEP_SUMMARY", os.devnull)
    try:
        n, kind, id_, entry, revoked = decide(os.environ)
    except Refused as e:
        message = f"Not an owner update that merges by itself: {e}. It waits for a reviewer."
        print(message)
        with open(summary, "a") as f:
            f.write(message + "\n")
        return 0
    with open(os.environ.get("GITHUB_OUTPUT", os.devnull), "a") as f:
        f.write(f"pr={n}\nsha={os.environ['HEAD_SHA']}\nkind={KINDS[kind]}\nid={id_}\nentry={entry}\n"
                f"revoked={revoked}\n")
    with open(summary, "a") as f:
        f.write(f"#{n}: an owner's update of {KINDS[kind]}; every rule holds.\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
