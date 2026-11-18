import contextlib
import io
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest

import catalogue


NATIVE = '''id = "sample"
kind = "native-mod"
name = "Sample <native>"
owners = ["Author"]
owner_ids = [7]
key = "original author key"
description = "<script>not markup</script>"
[[versions]]
version = "1.0.0"
file = "sample-1.0.0.hnmod"
sha256 = "{}"
signature = "original signature"
'''.format("a" * 64)
SHADER = NATIVE.replace('"native-mod"', '"shader"').replace(".hnmod", ".hshader")
PACK = NATIVE.replace('"native-mod"', '"modpack"').replace(".hnmod", ".hpack")
NATIVE_V1 = catalogue.ARCHIVE
API_01 = catalogue.ARCHIVES["api-0.1"][0]
# A reactivation: a genuinely new package with the archived identity.
PORT = NATIVE.replace("1.0.0", "2.0.0").replace(".hnmod", ".hmod").replace("a" * 64, "b" * 64)


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout.strip()


class CatalogueTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.base, self.head = self.root / "base", self.root / "head"
        for repo in (self.base, self.head):
            repo.mkdir()
            git(repo, "init", "-q")
            git(repo, "config", "user.email", "test@example.invalid")
            git(repo, "config", "user.name", "test")
            git(repo, "config", "core.symlinks", "true")
        self.moves = set()

    def write(self, repo, folder, text=NATIVE, name="sample.toml"):
        path = repo / folder / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def commit(self, repo):
        git(repo, "add", "-A")
        git(repo, "commit", "-q", "--allow-empty", "-m", "state")
        return git(repo, "rev-parse", "HEAD")

    def review(self, author_id=7):
        if self.moves:
            self.write(self.base, ".github", "".join(f"{m} {i}\n" for m, i in sorted(self.moves)), "archive-moves.txt")
        base, head = self.commit(self.base), self.commit(self.head)
        out = self.root / "review-base"
        if out.exists():
            for path in sorted(out.rglob("*"), reverse=True):
                path.rmdir() if path.is_dir() else path.unlink()
            out.rmdir()
        with contextlib.redirect_stdout(io.StringIO()):
            self.unconfirmed = catalogue.review_base(self.base, base, self.head, head, out, author_id)
        return out

    def assert_owner_unconfirmed(self, author_id):
        # Deferred: everything else is reviewed and the baseline written; only the owner is left to a maintainer.
        out = self.review(author_id)
        self.assertTrue((out / "mods").is_dir())
        self.assertEqual(len(self.unconfirmed), 1)
        self.assertIn("a maintainer must confirm the owner by hand", self.unconfirmed[0])

    def move(self, text=NATIVE, archive="native-v1", listed=True):
        self.write(self.base, "mods", text)
        self.write(self.head, catalogue.ARCHIVES[archive][0], text)
        if listed:
            self.moves.add((archive, "sample"))

    def test_reviewed_byte_preserving_move_leaves_normal_review_baseline(self):
        self.move()
        out = self.review()
        self.assertFalse((out / "mods/sample.toml").exists())

    def test_move_not_on_the_reviewed_list_is_refused(self):
        self.move(listed=False)
        with self.assertRaisesRegex(ValueError, "not on the reviewed list"):
            self.review()

    def test_reviewed_list_names_the_archive(self):
        self.move()
        self.moves = {("api-0.1", "sample")}
        with self.assertRaisesRegex(ValueError, "not on the reviewed list"):
            self.review()

    def test_mixed_extension_history_is_frozen_verbatim(self):
        historic = NATIVE + '''
[[versions]]
version = "0.1.0"
file = "sample-0.1.0.hmod"
sha256 = "{}"
'''.format("b" * 64)
        self.move(historic)
        self.review()

    def test_edited_entry_cannot_be_archived(self):
        self.move()
        self.write(self.head, NATIVE_V1, NATIVE.replace("original signature", "other signature"))
        with self.assertRaisesRegex(ValueError, "unchanged"):
            self.review()

    def test_archive_cannot_change_or_disappear_later(self):
        self.write(self.base, NATIVE_V1)
        with self.assertRaisesRegex(ValueError, "must not change or disappear"):
            self.review()

    def test_dependent_modpack_can_move_unchanged(self):
        self.move(PACK)
        self.review()

    def test_shader_api_01_pack_moves_to_its_own_archive(self):
        self.move(SHADER, "api-0.1")
        out = self.review()
        self.assertFalse((out / "mods/sample.toml").exists())

    def test_shader_is_not_native_history(self):
        self.move(SHADER)
        with self.assertRaisesRegex(ValueError, "native-v1 history is only"):
            self.review()

    def test_id_is_frozen_in_one_archive_only(self):
        self.move(SHADER, "api-0.1")
        self.write(self.head, NATIVE_V1)
        with self.assertRaisesRegex(ValueError, "one archive only"):
            self.review()

    def frozen(self, text=NATIVE, archive=NATIVE_V1):
        self.write(self.base, archive, text)
        self.write(self.head, archive, text)

    def test_owner_can_reactivate_with_a_new_package(self):
        self.frozen()
        self.write(self.head, "mods", PORT)
        self.review(author_id=7)
        self.assertEqual(self.unconfirmed, [])

    def test_unconfirmed_owner_does_not_hide_other_problems(self):
        self.frozen()
        self.write(self.head, "mods", NATIVE.replace(".hnmod", ".hmod"))
        with self.assertRaisesRegex(ValueError, "new version and package"):
            self.review(author_id=99)

    def test_cli_writes_unconfirmed_owners_and_succeeds(self):
        self.frozen()
        self.write(self.head, "mods", PORT)
        base, head = self.commit(self.base), self.commit(self.head)
        report = self.root / "owners.txt"
        result = subprocess.run(
            [sys.executable, str(pathlib.Path(catalogue.__file__)), "review-base", str(self.base), base,
             str(self.head), head, str(self.root / "cli-out"), "--author-id", "99", "--unconfirmed-owners", str(report)],
            capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("sample: this id is archived", report.read_text())

    def test_someone_else_cannot_reactivate_an_archived_id(self):
        self.frozen()
        self.write(self.head, "mods", PORT)
        self.assert_owner_unconfirmed(99)

    def test_entry_without_owner_ids_always_goes_to_a_maintainer(self):
        self.frozen(NATIVE.replace("owner_ids = [7]", ""))
        self.write(self.head, "mods", PORT.replace("owner_ids = [7]", ""))
        self.assert_owner_unconfirmed(7)

    def test_archived_shader_id_stays_reserved(self):
        self.frozen(SHADER, API_01)
        port = SHADER.replace("1.0.0", "2.0.0").replace("a" * 64, "b" * 64)
        self.write(self.head, "mods", port)
        self.review()
        self.write(self.head, "mods", port)
        self.assert_owner_unconfirmed(1)

    def test_reactivation_cannot_take_over_an_archived_id(self):
        self.frozen()
        self.write(self.head, "mods", PORT.replace('["Author"]', '["attacker"]'))
        with self.assertRaisesRegex(ValueError, "preserve archived owners"):
            self.review(author_id=7)

    def test_native_can_come_back_as_a_sandboxed_mod(self):
        self.frozen()
        self.write(self.head, "mods", PORT.replace('"native-mod"', '"mod"'))
        self.review(author_id=7)
        self.assertEqual(self.unconfirmed, [])

    def test_sandboxed_port_still_needs_its_owner(self):
        self.frozen()
        self.write(self.head, "mods", PORT.replace('"native-mod"', '"mod"'))
        self.assert_owner_unconfirmed(99)

    def test_sandboxed_port_cannot_take_over_an_archived_id(self):
        self.frozen()
        self.write(self.head, "mods", PORT.replace('"native-mod"', '"mod"').replace('["Author"]', '["attacker"]'))
        with self.assertRaisesRegex(ValueError, "preserve archived owners"):
            self.review(author_id=7)

    def assert_kind_change_refused(self, text, archive, old, new):
        self.frozen(text, archive)
        port = text.replace(f'"{old}"', f'"{new}"').replace("1.0.0", "2.0.0").replace("a" * 64, "b" * 64)
        self.write(self.head, "mods", port)
        with self.assertRaisesRegex(ValueError, "preserve archived kind"):
            self.review(author_id=7)

    def test_native_cannot_come_back_as_another_kind(self):
        self.assert_kind_change_refused(NATIVE, NATIVE_V1, "native-mod", "modpack")

    def test_archived_modpack_cannot_come_back_as_a_mod(self):
        self.assert_kind_change_refused(PACK, NATIVE_V1, "modpack", "mod")

    def test_archived_shader_cannot_come_back_as_a_mod(self):
        self.assert_kind_change_refused(SHADER, API_01, "shader", "mod")

    def test_reactivation_cannot_rename_historic_bytes(self):
        self.frozen()
        self.write(self.head, "mods", NATIVE.replace(".hnmod", ".hmod"))
        with self.assertRaisesRegex(ValueError, "new version and package"):
            self.review()

    def test_unchanged_reactivated_entry_needs_no_owner(self):
        self.frozen()
        self.write(self.base, "mods", PORT)
        self.write(self.head, "mods", PORT)
        self.review(author_id=1)

    @unittest.skipUnless(os.name == "posix", "needs symlinks")
    def test_linked_folder_is_refused(self):
        (self.root / "elsewhere").mkdir()
        self.write(self.root, "elsewhere")
        (self.head / "mods").symlink_to(self.root / "elsewhere")
        with self.assertRaisesRegex(ValueError, "mods: expected a folder"):
            self.review()

    @unittest.skipUnless(os.name == "posix", "needs symlinks")
    def test_linked_entry_is_refused(self):
        (self.head / "mods").mkdir()
        (self.head / "mods/sample.toml").symlink_to("/etc/hostname")
        with self.assertRaisesRegex(ValueError, "expected a regular <id>.toml file"):
            self.review()

    def test_id_syntax_is_enforced(self):
        self.write(self.head, "mods", NATIVE.replace('"sample"', '"Sample"'), "Sample.toml")
        with self.assertRaisesRegex(ValueError, "expected a regular <id>.toml file"):
            self.review()

    def test_history_is_explicitly_noninstallable_and_html_escaped(self):
        self.write(self.head, NATIVE_V1)
        sha = self.commit(self.head)
        out = self.root / "history"
        catalogue.history(self.head, sha, out)
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
