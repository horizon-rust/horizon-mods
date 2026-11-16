import contextlib
import io
import json
import pathlib
import tempfile
import unittest

import catalogue


NATIVE = '''id = "sample"
kind = "native-mod"
name = "Sample <native>"
owners = ["author"]
owner_ids = [7]
key = "original author key"
description = "<script>not markup</script>"
[[versions]]
version = "1.0.0"
file = "sample-1.0.0.hnmod"
sha256 = "{}"
signature = "original signature"
'''.format("a" * 64)


class CatalogueTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.base, self.head = self.root / "base", self.root / "head"
        for path in (self.base, self.head):
            (path / "mods").mkdir(parents=True)

    def write(self, root, folder, text=NATIVE):
        path = root / folder / "sample.toml"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def review(self):
        with contextlib.redirect_stdout(io.StringIO()):
            catalogue.review_base(self.base, self.head, self.root / "review-base")

    def freeze(self):
        self.write(self.base, "mods")
        self.write(self.head, catalogue.ARCHIVE)

    def test_byte_preserving_move_leaves_normal_review_baseline(self):
        self.freeze()
        self.review()
        self.assertFalse((self.root / "review-base/mods/sample.toml").exists())
        self.assertEqual((self.head / catalogue.ARCHIVE / "sample.toml").read_text(), NATIVE)

    def test_mixed_extension_history_is_frozen_verbatim(self):
        historic = NATIVE + '''
[[versions]]
version = "0.1.0"
file = "sample-0.1.0.hmod"
sha256 = "{}"
'''.format("b" * 64)
        self.write(self.base, "mods", historic)
        self.write(self.head, catalogue.ARCHIVE, historic)
        self.review()
        self.assertEqual((self.head / catalogue.ARCHIVE / "sample.toml").read_text(), historic)
        self.assertFalse((self.root / "review-base/mods/sample.toml").exists())

    def test_edited_entry_cannot_be_archived(self):
        self.freeze()
        self.write(self.head, catalogue.ARCHIVE, NATIVE.replace("original signature", "other signature"))
        with self.assertRaisesRegex(ValueError, "unchanged"):
            self.review()

    def test_archive_cannot_change_or_disappear_later(self):
        self.write(self.base, catalogue.ARCHIVE)
        with self.assertRaisesRegex(ValueError, "must not change or disappear"):
            self.review()

    def test_old_extension_is_never_live(self):
        self.write(self.head, "mods")
        with self.assertRaisesRegex(ValueError, "not a live package"):
            self.review()

    def reactivate(self):
        self.write(self.base, catalogue.ARCHIVE)
        self.write(self.head, catalogue.ARCHIVE)
        return NATIVE.replace("1.0.0", "2.0.0").replace(".hnmod", ".hmod").replace("a" * 64, "b" * 64)

    def test_reactivation_preserves_identity_and_needs_new_package(self):
        current = self.reactivate()
        self.write(self.head, "mods", current)
        self.review()

    def test_reactivation_cannot_take_over_an_archived_id(self):
        current = self.reactivate()
        self.write(self.head, "mods", current.replace('["author"]', '["attacker"]'))
        with self.assertRaisesRegex(ValueError, "preserve archived owners"):
            self.review()

    def test_reactivation_cannot_rename_historic_bytes(self):
        self.reactivate()
        self.write(self.head, "mods", NATIVE.replace(".hnmod", ".hmod"))
        with self.assertRaisesRegex(ValueError, "new version and package"):
            self.review()

    def test_dependent_modpack_can_move_unchanged(self):
        pack = NATIVE.replace('"native-mod"', '"modpack"').replace(".hnmod", ".hpack")
        self.write(self.base, "mods", pack)
        self.write(self.head, catalogue.ARCHIVE, pack)
        self.review()

    def test_history_is_explicitly_noninstallable_and_html_escaped(self):
        self.write(self.head, catalogue.ARCHIVE)
        out = self.root / "history"
        catalogue.history(self.head, out)
        data = json.loads((out / "catalogue.json").read_text())
        self.assertFalse(data["installable"])
        self.assertNotIn("schema", data)
        page = (out / "index.html").read_text()
        self.assertIn("incompatible", page)
        self.assertIn("&lt;script&gt;", page)
        self.assertNotIn("<script>", page)
        self.assertIn("sample-1.0.0/sample-1.0.0.hnmod", page)
        self.assertEqual((out / "mods/sample.toml").read_text(), NATIVE)


if __name__ == "__main__":
    unittest.main()
