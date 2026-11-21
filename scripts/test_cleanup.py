import importlib.util
import json
import os
import pathlib
import subprocess
import tempfile
import unittest

# Loaded by path: `python3 -I` leaves this folder off sys.path.
spec = importlib.util.spec_from_file_location("cleanup", pathlib.Path(__file__).with_name("cleanup.py"))
cleanup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cleanup)

DAY = 86400
NOW = 1_800_000_000


def sha(n):
    return f"{n:064x}"


def created(days):
    when = cleanup.datetime.datetime.fromtimestamp(NOW - days * DAY, cleanup.datetime.UTC)
    return when.strftime("%Y-%m-%dT%H:%M:%SZ")


def version(v, file, h, quote='"'):
    return f'[[versions]]\nversion = "{v}"\nfile = "{file}"\nsha256 = {quote}{h}{quote}\n'


class Plan(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = pathlib.Path(temp.name)
        self.git("init", "-q")
        os.environ["GIT_DIR"] = str(self.root / ".git")
        self.addCleanup(os.environ.pop, "GIT_DIR")
        pack = version("1.0.0", "pack.zip", sha(1)) + version("2.0.0", "pack2.zip", sha(2))
        self.main = self.commit({
            "mods/pack.toml": 'id = "pack"\n' + pack,
            "archive/api-0.1/mods/old.toml": version("1.0.0", "old.hnmod", sha(3)),
        })
        self.heads = []
        self.releases = [{"tag_name": "pack-1.0.0", "draft": False,
                          "assets": [{"name": "pack.zip", "state": "uploaded", "digest": f"sha256:{sha(1)}"}]},
                         {"tag_name": "staging", "draft": False,
                          "assets": [{"name": "pack2.zip", "digest": f"sha256:{sha(2)}"}]}]

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.root), "-c", "user.name=t", "-c", "user.email=t@t", *args],
                              check=True, capture_output=True, text=True).stdout.strip()

    def commit(self, files):
        for path, text in files.items():
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text, newline="\n")
        self.git("add", "-A")
        self.git("commit", "-qm", "c", "--allow-empty")
        return self.git("rev-parse", "HEAD")

    def plan(self, *assets):
        rows = [[str(n), name, created(days)] for n, (name, days) in enumerate(assets)]
        text = "".join(json.dumps(r) + "\n" for r in self.releases)
        out, _ = cleanup.plan(NOW, self.main, rows, text, self.heads)
        return {name: why for _, name, why in out}

    def test_published_goes_after_a_day_once_its_release_has_it(self):
        self.assertEqual(self.plan((f"{sha(1)}.zip", 0.5)), {})
        self.assertEqual(self.plan((f"{sha(1)}.zip", 1)), {f"{sha(1)}.zip": "deleted: published in its release"})

    def test_published_without_its_release_is_kept(self):
        # pack-2.0.0 has no release yet: staging's is the only copy fetch.sh can find.
        self.assertEqual(self.plan((f"{sha(2)}.zip", 30)), {})

    def test_another_file_digest_or_state_or_a_draft_doesnt_count(self):
        good = {"name": "pack.zip", "state": "uploaded", "digest": f"sha256:{sha(1)}"}
        for asset, draft in (({**good, "name": "other.zip"}, False), ({**good, "digest": None}, False),
                             ({**good, "state": "starter"}, False), (good, True)):
            with self.subTest(asset=asset, draft=draft):
                self.releases[0] = {"tag_name": "pack-1.0.0", "draft": draft, "assets": [asset]}
                self.assertEqual(self.plan((f"{sha(1)}.zip", 30)), {})

    def test_every_record_with_the_hash_needs_its_release(self):
        self.main = self.commit({"mods/twin.toml": version("1.0.0", "pack.zip", sha(1))})
        self.assertEqual(self.plan((f"{sha(1)}.zip", 30)), {})

    def test_abandoned_goes_after_a_week_and_a_retired_hnmod_at_once(self):
        self.assertEqual(self.plan((f"{sha(9)}.hmod", 6.9), (f"{sha(8)}.hmod", 7), (f"{sha(7)}.hnmod", 0)),
                         {f"{sha(8)}.hmod": "deleted: abandoned",
                          f"{sha(7)}.hnmod": "deleted: abandoned, retired format"})

    def test_open_pull_requests_and_archive_keep_theirs(self):
        self.heads = [self.commit({"mods/new.toml": version("1.0.0", "new.hnmod", sha(9))})]
        self.assertEqual(self.plan((f"{sha(9)}.hnmod", 60), (f"{sha(3)}.hnmod", 60)), {})

    def test_a_pull_request_keeps_only_records_without_their_releases_copy(self):
        # Every pull request carries main's pack.toml: its released 1.0.0 doesn't keep staging's copy. The same
        # bytes as a new version, or in another id, have no release of their own yet: fetch.sh reads staging.
        main = (self.root / "mods" / "pack.toml").read_text()
        self.heads = [self.commit({"mods/pack.toml": main + version("1.0.1", "pack.zip", sha(1))})]
        self.assertEqual(self.plan((f"{sha(1)}.zip", 30)), {})
        self.git("reset", "-q", "--hard", self.main)
        self.heads = [self.commit({"mods/other.toml": version("1.0.0", "pack.zip", sha(1))})]
        self.assertEqual(self.plan((f"{sha(1)}.zip", 30)), {})
        self.git("reset", "-q", "--hard", self.main)
        self.heads = [self.commit({"README.md": "x"})]
        self.assertEqual(self.plan((f"{sha(1)}.zip", 30)), {f"{sha(1)}.zip": "deleted: published in its release"})

    def test_only_a_pull_requests_entries_keep_files(self):
        self.heads = [self.commit({"README.md": version("1.0.0", "x.hmod", sha(9)),
                                   "mods/sub/x.toml": version("1.0.0", "x.hmod", sha(8))})]
        self.assertEqual(set(self.plan((f"{sha(9)}.hmod", 60), (f"{sha(8)}.hmod", 60))),
                         {f"{sha(9)}.hmod", f"{sha(8)}.hmod"})

    def test_any_way_of_writing_the_hash_keeps_it(self):
        # Single quotes, a unicode escape, and an entry that isn't valid TOML: in main's mods/ (no release yet),
        # main's archive/ and an open pull request.
        escaped = sha(10)[:-1] + "\\u00" + sha(10)[-1].encode().hex()
        broken = version("1.0.0", "b.hmod", sha(6)) + "this is not toml\n"
        self.main = self.commit({"mods/single.toml": version("1.0.0", "s.hmod", sha(4), "'"),
                                 "mods/escaped.toml": version("1.0.0", "e.hmod", escaped),
                                 "archive/native-v1/mods/single.toml": version("1.0.0", "a.hnmod", sha(5), "'"),
                                 "mods/broken.toml": broken})
        self.heads = [self.commit({"mods/pr.toml": version("1.0.0", "p.hnmod", sha(7), "'") + "[broken\n"})]
        self.assertEqual(self.plan((f"{sha(4)}.hmod", 60), (f"{sha(10)}.hmod", 60), (f"{sha(5)}.hnmod", 60),
                                   (f"{sha(6)}.hmod", 60), (f"{sha(7)}.hnmod", 60)), {})

    def test_malformed_versions_field_does_not_crash_and_keeps_hashes(self):
        # A PR with a non-list `versions` field parses as TOML; it must not crash cleanup and must keep its hash.
        self.heads = [self.commit({"mods/bad.toml": f'versions = 1\nsha256 = "{sha(11)}"\n'})]
        self.assertEqual(self.plan((f"{sha(11)}.hmod", 60)), {})

    def test_broken_main_entry_protects_hash_even_when_another_entry_is_released(self):
        # mods/pack.toml already has a verified release for sha(1), but a broken main entry sharing sha(1)
        # cannot be verified and must independently keep staging's copy.
        self.main = self.commit({"mods/broken_twin.toml": version("1.0.0", "twin.zip", sha(1)) + "[broken\n"})
        self.assertEqual(self.plan((f"{sha(1)}.zip", 30)), {})

    def test_oldest_first(self):
        rows = [["1", f"{sha(8)}.hmod", created(8)], ["2", f"{sha(7)}.hmod", created(9)]]
        out, _ = cleanup.plan(NOW, self.main, rows, "", [])
        self.assertEqual([r[0] for r in out], ["2", "1"])


if __name__ == "__main__":
    unittest.main()
