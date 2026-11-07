import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_build  # noqa: E402

BASE = "656064105c0e1fbd0a8f1d6e5f5d3a8b3c2e1f00"


def version(v, sha, broken=()):
    return {"version": v, "file": {"url": f"https://x/{v}", "sha256": sha},
            "broken_on": [{"horizon": b, "code": "E"} for b in broken]}


INDEX = {
    "projects": [
        {"id": "zoom", "kind": "native-mod", "versions": [version("2.1.0", "c" * 64), version("2.0.0", "b" * 64)]},
        {"id": "minimap", "kind": "native-mod", "versions": [version("1.4.0", "a" * 64)]},
        {"id": "broken", "kind": "native-mod", "versions": [version("1.0.0", "d" * 64, [BASE])]},
        {"id": "pack", "kind": "hmod", "versions": [version("1.0.0", "e" * 64)]},
    ],
    "revoked": [{"id": "zoom", "sha256": "c" * 64}],
}


class Key(unittest.TestCase):
    def test_the_key_is_the_launchers(self):
        # The same vector as horizon_release_sign::native_build's test: CI and the launcher name a build alike.
        mods = [("zoom", "2.0.0", "b" * 64), ("minimap", "1.4.0", "a" * 64)]
        self.assertEqual(native_build.request_key("x86_64-unknown-linux-gnu", BASE, mods),
                         "c07fffc296cf14c5c496a016a930f84b0d0ac028a63c667c2bdf47657788445d")

    def test_names(self):
        key = "0123456789abcdef" + "0" * 48
        self.assertEqual(native_build.release_tag(BASE), "native-builds-656064105c0e")
        self.assertEqual(native_build.archive(key, "aarch64-apple-darwin"),
                         "0123456789abcdef-aarch64-apple-darwin.tar.gz")
        self.assertEqual(native_build.signature(key, "aarch64-apple-darwin"),
                         "0123456789abcdef-aarch64-apple-darwin.sig")


class Pick(unittest.TestCase):
    def test_newest_unrevoked_version_unless_one_is_named(self):
        picked = native_build.pick(INDEX, ["zoom", "minimap@1.4.0"], BASE)
        self.assertEqual([(p["id"], p["version"]) for p in picked], [("zoom", "2.0.0"), ("minimap", "1.4.0")])

    def test_what_the_launcher_would_not_build_is_refused(self):
        for wanted in (["nothing"], ["pack"], ["zoom@2.1.0"], ["zoom@9"], ["broken"], ["zoom", "zoom@2.0.0"], []):
            with self.assertRaises(ValueError, msg=wanted):
                native_build.pick(INDEX, wanted, BASE)

    def test_broken_on_another_commit_still_builds(self):
        picked = native_build.pick(INDEX, ["broken"], "1" * 40)
        self.assertEqual(picked[0]["version"], "1.0.0")


SETS = """
[[set]]
name = "Two"
mods = ["zoom", "minimap@1.4.0"]

[[set]]
name = "One"
mods = ["zoom"]
"""
LINUX = "x86_64-unknown-linux-gnu"
MAC = "aarch64-apple-darwin"
ZOOM = {"id": "zoom", "version": "2.0.0", "sha256": "b" * 64}
MINIMAP = {"id": "minimap", "version": "1.4.0", "sha256": "a" * 64}


def key(target, *mods):
    return native_build.request_key(target, BASE, [(m["id"], m["version"], m["sha256"]) for m in mods])


class OnlyListedSets(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.sets = Path(tmp.name) / "prebuilt.toml"
        self.sets.write_text(SETS)

    def test_plan_takes_a_listed_set_in_any_order(self):
        self.assertEqual(native_build.listed(["minimap@1.4.0", "zoom"], self.sets), ["minimap@1.4.0", "zoom"])

    def test_plan_refuses_any_other_set(self):
        for wanted in (["minimap"], ["zoom", "minimap"], ["zoom", "minimap@1.4.0", "keystrokes"], ["zoom@2.0.0"], []):
            with self.assertRaises(ValueError, msg=wanted):
                native_build.listed(wanted, self.sets)

    def test_sign_recomputes_each_rows_key(self):
        matrix = [{"target": LINUX, "key": key(LINUX, ZOOM, MINIMAP), "archive": "ignored"},
                  {"target": MAC, "key": key(MAC, ZOOM, MINIMAP)}]
        self.assertEqual(native_build.signable(BASE, [ZOOM, MINIMAP], matrix, self.sets),
                         [(key(LINUX, ZOOM, MINIMAP), LINUX), (key(MAC, ZOOM, MINIMAP), MAC)])

    def test_sign_refuses_another_sets_key(self):
        # A row naming the key of `zoom` alone, in a run of the two-mod set: H1 of the review.
        with self.assertRaises(ValueError):
            native_build.signable(BASE, [ZOOM, MINIMAP], [{"target": LINUX, "key": key(LINUX, ZOOM)}], self.sets)
        # Another target's key on this row.
        with self.assertRaises(ValueError):
            native_build.signable(BASE, [ZOOM], [{"target": LINUX, "key": key(MAC, ZOOM)}], self.sets)

    def test_sign_refuses_mods_or_rows_outside_the_rules(self):
        good = [{"target": LINUX, "key": key(LINUX, ZOOM)}]
        cases = [
            ("1" * 39, [ZOOM], good),                                     # not a commit
            (BASE, [], good),                                             # no mods
            (BASE, [dict(ZOOM, id="zoom\nmod x")], good),                # a line break in an id
            (BASE, [dict(ZOOM, version="2.0 0")], good),                  # a space in a version
            (BASE, [dict(ZOOM, sha256="B" * 64)], good),                  # not lowercase hex
            (BASE, [MINIMAP], [{"target": LINUX, "key": key(LINUX, MINIMAP)}]),  # minimap alone isn't listed
            (BASE, [dict(MINIMAP, version="1.5.0"), ZOOM],                # the set pins minimap 1.4.0
             [{"target": LINUX, "key": key(LINUX, dict(MINIMAP, version="1.5.0"), ZOOM)}]),
            (BASE, [ZOOM], good + good),                                  # a target twice
            (BASE, [ZOOM], [{"target": "x86_64-unknown-freebsd", "key": key("x86_64-unknown-freebsd", ZOOM)}]),
            (BASE, [ZOOM], {"target": LINUX}),                            # not a list
        ]
        for base, mods, matrix in cases:
            with self.assertRaises(ValueError, msg=(base, mods, matrix)):
                native_build.signable(base, mods, matrix, self.sets)


class Sets(unittest.TestCase):
    def test_the_registrys_sets_are_native_mods_it_lists(self):
        lines = native_build.sets(native_build.ROOT / "prebuilt.toml")
        self.assertTrue(lines)
        official = {p.name for p in (native_build.ROOT / "native").iterdir() if (p / "compatibility.json").is_file()}
        for line in lines:
            for item in line.split():
                self.assertIn(item.partition("@")[0].replace("_", "-"), official, line)


if __name__ == "__main__":
    unittest.main()
