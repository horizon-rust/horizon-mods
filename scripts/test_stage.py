import contextlib
import hashlib
import importlib.util
import io
import os
import pathlib
import subprocess
import tempfile
import unittest

# Loaded by path: `python3 -I` leaves this folder off sys.path.
spec = importlib.util.spec_from_file_location("stage", pathlib.Path(__file__).with_name("stage.py"))
stage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stage)

BYTES = b"a resource pack's bytes"
SHA = hashlib.sha256(BYTES).hexdigest()
ENTRY = 'id = "pack"\nkind = "resourcepack"\nowner_ids = [1]\n'
OLD = '[[versions]]\nversion = "1.0.0"\nfile = "pack-1.0.0.zip"\nsha256 = "' + "0" * 64 + '"\n'
NEW = f'[[versions]]\nversion = "1.1.0"\nfile = "pack-1.1.0.zip"\nsha256 = "{SHA}"\n'
LINK = "https://github.com/someone/pack/releases/download/v1.1.0/pack-1.1.0.zip"
KEY = "RW" + "Q" * 54


class Links(unittest.TestCase):
    def test_markdown_angle_and_plain_links_are_found(self):
        for body in (f"[file]({LINK})", f"<{LINK}>", f"File: {LINK}.", f"{LINK}, or {LINK}\n"):
            with self.subTest(body=body):
                self.assertEqual(stage.link(body, "pack-1.1.0.zip"), LINK)

    def test_an_encoded_name_is_its_file(self):
        url = "https://github.com/someone/pack/releases/download/v1/pack%2Bmc.zip"
        self.assertEqual(stage.link(url, "pack+mc.zip"), url)

    def test_refused(self):
        cases = {
            "": "no link",
            "https://github.com/someone/pack/releases/download/v1/other.zip": "no link",
            "https://example.com/pack-1.1.0.zip": "not a GitHub release download",
            "https://github.com/someone/pack/raw/main/pack-1.1.0.zip": "not a GitHub release download",
            f"{LINK} https://github.com/other/pack/releases/download/v1/pack-1.1.0.zip": "2 different links",
        }
        for body, why in cases.items():
            with self.subTest(body=body), self.assertRaisesRegex(stage.Refused, why):
                stage.link(body, "pack-1.1.0.zip")

    def test_download_refuses_other_hosts_before_connecting(self):
        for url in ("https://example.com/x.zip", "http://github.com/x.zip", "https://github.com:8443/x.zip"):
            with self.subTest(url=url), self.assertRaisesRegex(stage.Refused, "GitHub's asset hosts"):
                stage.download(url, os.devnull)


class Records(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = pathlib.Path(temp.name)
        self.git("init", "-q")
        os.environ["GIT_DIR"] = str(self.root / ".git")
        self.addCleanup(os.environ.pop, "GIT_DIR")
        self.base = self.commit({"mods/pack.toml": ENTRY + OLD, "mods/other.toml": ENTRY + OLD})

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.root), "-c", "user.name=t", "-c", "user.email=t@t", *args],
                              check=True, capture_output=True, text=True).stdout.strip()

    def commit(self, files):
        for path, text in files.items():
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            if text is None:
                target.unlink()
            else:
                target.write_text(text, newline="\n")
        self.git("add", "-A")
        self.git("commit", "-qm", "c", "--allow-empty")
        return self.git("rev-parse", "HEAD")

    def test_only_new_records(self):
        head = self.commit({"mods/pack.toml": ENTRY + NEW + OLD, "mods/other.toml": None, "README.md": "x"})
        new = stage.added(self.base, head)
        self.assertEqual([(r["path"], r["version"], r["name"]) for r in new],
                         [("mods/pack.toml", "1.1.0", f"{SHA}.zip")])
        self.assertEqual(new[0]["kind"], "resourcepack")

    def test_a_page_change_has_nothing_to_stage(self):
        head = self.commit({"mods/pack.toml": ENTRY.replace("[1]", "[2]") + OLD})
        self.assertEqual(stage.added(self.base, head), [])
        files = self.root / "files"
        files.mkdir()
        body = self.root / "body"
        body.write_text("")
        self.assertEqual(self.run_main("links", self.base, head, str(body), str(files)), 0)
        self.assertEqual((self.root / "files.json").read_text(), "[]\n")
        with self.assertRaisesRegex(stage.Refused, "adds no"):
            self.run_main("download", self.base, head, LINK, str(files))

    def test_a_record_that_is_not_strings_is_refused(self):
        head = self.commit({"mods/pack.toml": ENTRY + OLD + NEW.replace(f'"{SHA}"', "1")})
        with self.assertRaisesRegex(stage.Refused, "sha256 must be a string"):
            stage.added(self.base, head)

    def test_a_link_is_not_an_entry(self):
        self.git("rm", "-q", "mods/pack.toml")
        blob = subprocess.run(["git", "-C", str(self.root), "hash-object", "-w", "--stdin"], input=b"/etc/passwd",
                              capture_output=True, check=True).stdout.decode().strip()
        self.git("update-index", "--add", "--cacheinfo", f"120000,{blob},mods/pack.toml")
        self.git("commit", "-qm", "link")
        with self.assertRaisesRegex(stage.Refused, "not a regular file"):
            stage.added(self.base, self.git("rev-parse", "HEAD"))

    def run_main(self, *args):
        with contextlib.redirect_stdout(io.StringIO()):
            return stage.main([*args])

    def file(self, name, text):
        path = self.root / name
        path.write_text(text)
        return str(path)

    def test_verify_takes_only_new_records_files(self):
        head = self.commit({"mods/pack.toml": ENTRY + NEW + OLD})
        files = self.root / "files"
        files.mkdir()
        none = self.file("none", "")
        (files / f"{SHA}.zip").write_bytes(BYTES)
        self.assertEqual(self.run_main("verify", self.base, head, str(files), none), 0)
        (files / f"{SHA}.zip").write_bytes(b"other bytes")
        with self.assertRaisesRegex(stage.Refused, "SHA-256 is not"):
            self.run_main("verify", self.base, head, str(files), none)
        (files / f"{SHA}.zip").unlink()
        (files / f"{'0' * 64}.zip").write_bytes(BYTES)
        with self.assertRaisesRegex(stage.Refused, "not a new version's"):
            self.run_main("verify", self.base, head, str(files), none)

    def test_verify_needs_every_new_file_staged_or_downloaded(self):
        # A failed artifact download leaves the folder empty: that is not "nothing to upload".
        head = self.commit({"mods/pack.toml": ENTRY + NEW + OLD})
        files = self.root / "files"
        files.mkdir()
        with self.assertRaisesRegex(stage.Refused, "neither staged nor downloaded: mods/pack.toml version 1.1.0"):
            self.run_main("verify", self.base, head, str(files), self.file("none", ""))
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(stage.main(["verify", self.base, head, str(files), self.file("staged", f"{SHA}.zip\n")]),
                             0)
        self.assertEqual(out.getvalue(), "")

    def test_a_key_must_be_a_string_and_any_key_needs_signatures(self):
        for key in ('""', '"  "', "1", "[]"):
            head = self.commit({"mods/pack.toml": ENTRY + f"key = {key}\n" + NEW + OLD})
            with self.subTest(key=key), self.assertRaisesRegex(stage.Refused, "not a minisign public key"):
                stage.added(self.base, head)
        head = self.commit({"mods/pack.toml": ENTRY + f'key = "{KEY}"\n' + NEW + OLD})
        files = self.root / "files"
        files.mkdir()
        stage.plan(files, [{**r, "staged": False} for r in stage.added(self.base, head)])
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(stage.check(files), 1)
        self.assertIn("needs a signature", out.getvalue())

    def test_ready_only_when_the_check_missed_files_that_are_staged_now(self):
        head = self.commit({"mods/pack.toml": ENTRY + NEW + OLD})
        missed = '"pack 1.1.0: reviewed package is absent from its release and staging"\n'
        exit1 = '"Process completed with exit code 1."\n'
        staged, none = self.file("staged", f"{SHA}.zip\n"), self.file("none", "")
        cases = [
            (missed + exit1, staged, 0),
            (exit1, staged, 1),  # failed on something else
            (missed, none, 1),  # not staged yet
            (missed.replace("pack 1.1.0", "other 1.0.0"), staged, 1),  # another entry's file
            (missed + missed.replace("pack 1.1.0", "other 1.0.0"), staged, 1),
            ('"pack 1.1.0: reviewed package is absent"\n', staged, 1),
        ]
        for failures, have, want in cases:
            with self.subTest(failures=failures, have=have):
                self.assertEqual(self.run_main("ready", self.base, head, have, self.file("failures", failures)), want)

    def test_links_stages_at_most_a_few_and_skips_staged(self):
        many = "".join(NEW.replace("1.1.0", f"1.1.{n}") for n in range(stage.MAX_FILES + 1))
        head = self.commit({"mods/pack.toml": ENTRY + many + OLD})
        body = self.root / "body"
        body.write_text("")
        files = self.root / "files"
        files.mkdir()
        with self.assertRaisesRegex(stage.Refused, "at most"):
            self.run_main("links", self.base, head, str(body), str(files))
        staged = self.root / "staged"
        staged.write_text(f"{SHA}.zip\n")
        self.assertEqual(self.run_main("links", self.base, head, str(body), str(files), str(staged)), 0)
        self.assertEqual(list(files.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
