import importlib.util
import json
import pathlib
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


def version(v, file, h):
    return f'[[versions]]\nversion = "{v}"\nfile = "{file}"\nsha256 = "{h}"\n'


class Plan(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = pathlib.Path(temp.name)
        (self.root / "mods").mkdir()
        (self.root / "archive" / "api-0.1" / "mods").mkdir(parents=True)
        (self.root / "mods" / "pack.toml").write_text(
            'id = "pack"\n' + version("1.0.0", "pack.zip", sha(1)) + version("2.0.0", "pack2.zip", sha(2)))
        (self.root / "archive" / "api-0.1" / "mods" / "old.toml").write_text(version("1.0.0", "old.hnmod", sha(3)))
        self.releases = [{"tag_name": "pack-1.0.0", "draft": False,
                          "assets": [{"name": "pack.zip", "state": "uploaded", "digest": f"sha256:{sha(1)}"}]},
                         {"tag_name": "staging", "draft": False,
                          "assets": [{"name": "pack2.zip", "digest": f"sha256:{sha(2)}"}]}]

    def plan(self, *assets, pending=()):
        rows = [[str(n), name, created(days)] for n, (name, days) in enumerate(assets)]
        text = "".join(json.dumps(r) + "\n" for r in self.releases)
        out, _ = cleanup.plan(NOW, self.root, rows, text, set(pending))
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
        (self.root / "mods" / "twin.toml").write_text(version("1.0.0", "pack.zip", sha(1)))
        self.assertEqual(self.plan((f"{sha(1)}.zip", 30)), {})

    def test_abandoned_goes_after_a_week_and_a_retired_hnmod_at_once(self):
        self.assertEqual(self.plan((f"{sha(9)}.hmod", 6.9), (f"{sha(8)}.hmod", 7), (f"{sha(7)}.hnmod", 0)),
                         {f"{sha(8)}.hmod": "deleted: abandoned",
                          f"{sha(7)}.hnmod": "deleted: abandoned, retired format"})

    def test_pending_and_archived_are_kept(self):
        self.assertEqual(self.plan((f"{sha(9)}.hnmod", 60), (f"{sha(1)}.zip", 60), pending=[sha(9), sha(1)]), {})
        self.assertEqual(self.plan((f"{sha(3)}.hnmod", 60)), {})

    def test_oldest_first(self):
        rows = [["1", f"{sha(8)}.hmod", created(8)], ["2", f"{sha(7)}.hmod", created(9)]]
        out, _ = cleanup.plan(NOW, self.root, rows, "", set())
        self.assertEqual([r[0] for r in out], ["2", "1"])


if __name__ == "__main__":
    unittest.main()
