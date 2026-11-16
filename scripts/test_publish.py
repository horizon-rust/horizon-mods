import hashlib
import os
import pathlib
import re
import shutil
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


@unittest.skipUnless(os.name == "posix" and shutil.which("bash"), "publisher runs on a POSIX runner")
class PublishTests(unittest.TestCase):
    def test_shared_package_bytes_publish_without_replacing_an_existing_asset(self):
        workflow = (ROOT / ".github/workflows/publish.yml").read_text()
        scripts = re.findall(r"^        run: \|\n((?:          .*\n|\n)+)", workflow, re.MULTILINE)
        script = next("\n".join(line[10:] for line in block.splitlines())
                      for block in scripts if "gh release upload registry-files" in block)
        package = b"reviewed resource-pack bytes shared by two release versions"
        sha = hashlib.sha256(package).hexdigest()
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            for folder in ("bin", "files", "remote"):
                (root / folder).mkdir()
            (root / "files" / sha).write_bytes(package)
            cargo = root / "bin/cargo"
            cargo.write_text(f"#!/bin/sh\nprintf '%s\\n' '{sha} sample 1.0 sample-1.0.zip' '{sha} sample 1.1 sample-1.1.zip'\n")
            gh = root / "bin/gh"
            gh.write_text('''#!/bin/sh
set -eu
case "$1 $2 $3" in
  "release view registry-files") exit 0 ;;
  "release upload registry-files")
    destination="remote/${4##*/}"
    test ! -e "$destination" || exit 17
    cp "$4" "$destination" ;;
  *) exit 18 ;;
esac
''')
            cargo.chmod(0o755)
            gh.chmod(0o755)
            result = subprocess.run(
                ["bash", "-e", "-o", "pipefail", "-c", script], cwd=root,
                env={**os.environ, "PATH": f"{root / 'bin'}:{os.environ['PATH']}"},
                text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            )
            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertEqual((root / "remote" / sha).read_bytes(), package)


if __name__ == "__main__":
    unittest.main()
