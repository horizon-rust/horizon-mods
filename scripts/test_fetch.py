import hashlib
import os
import pathlib
import subprocess
import tempfile
import unittest


FETCH = pathlib.Path(__file__).with_name("fetch.sh")


class FetchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.source = self.root / "source"
        self.source.mkdir()
        self.package = b"bytes of the reviewed download; package inspection is a separate boundary"
        self.sha = hashlib.sha256(self.package).hexdigest()
        self.file = self.source / "sample-1.0.0.hmod"
        self.file.write_bytes(self.package)
        cargo = self.bin / "cargo"
        cargo.write_text(f"#!/bin/sh\nprintf '%s\\n' '{self.sha} sample 1.0.0 {self.file.name}'\n")
        cargo.chmod(0o755)

    def fetch(self):
        return subprocess.run(
            ["bash", str(FETCH), "."], cwd=self.root, text=True,
            env={**os.environ, "PATH": f"{self.bin}:{os.environ['PATH']}", "FILES_FROM": str(self.source)},
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )

    def test_download_is_hashed(self):
        result = self.fetch()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual((self.root / "files" / self.sha).read_bytes(), self.package)

    def test_all_accepted_plain_filename_shapes_are_fetched(self):
        for name in ("pack+mc26.zip", "+pack.zip", "-pack.zip", "_pack.zip", "p" * 124 + ".zip"):
            with self.subTest(name=name):
                self.file = self.file.rename(self.source / name)
                (self.bin / "cargo").write_text(
                    f"#!/bin/sh\nprintf '%s\\n' '{self.sha} sample 1.0.0 {name}'\n")
                destination = self.root / "files" / self.sha
                destination.unlink(missing_ok=True)
                result = self.fetch()
                self.assertEqual(result.returncode, 0, result.stdout)
                self.assertEqual(destination.read_bytes(), self.package)

    def test_existing_content_is_rehashed_and_corruption_replaced(self):
        destination = self.root / "files" / self.sha
        destination.parent.mkdir()
        destination.write_bytes(b"poisoned local bytes")
        result = self.fetch()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(destination.read_bytes(), self.package)

    def test_mismatch_refuses_without_committing_download(self):
        self.file.write_bytes(b"unreviewed replacement")
        result = self.fetch()
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("SHA-256 mismatch", result.stdout)
        self.assertFalse((self.root / "files" / self.sha).exists())


if __name__ == "__main__":
    unittest.main()
