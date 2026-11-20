import contextlib
import importlib.util
import io
import pathlib
import tempfile
import unittest
from unittest import mock

# Loaded by path: `python3 -I` leaves this folder off sys.path.
spec = importlib.util.spec_from_file_location("owner_update", pathlib.Path(__file__).with_name("owner_update.py"))
ou = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ou)

PATH = "mods/pack.toml"
TOP = 'id = "pack"\nkind = "resourcepack"\nname = "Pack"\nowners = ["alice"]\nowner_ids = [42]\n'
OLD = '\n[[versions]]\nversion = "1.0.0"\nfile = "pack-1.0.0.zip"\nsha256 = "' + "a" * 64 + '"\n'
NEW = '\n[[versions]]\nversion = "1.1.0"\nfile = "pack-1.1.0.zip"\nsha256 = "' + "b" * 64 + '"\n'
REVOKED = '[[revoked]]\nid = "other"\nreason = "r"\ndate = "2026-11-01"\n'


def run(base=TOP + OLD, head=TOP + NEW + OLD, main=None, revoked=REVOKED, author=42, changes=(("M", PATH),)):
    main = base if main is None else main
    return ou.eligible(list(changes), base.encode(), head.encode(), main.encode(), revoked.encode(), author)


class Eligible(unittest.TestCase):
    def test_an_owner_adding_versions_is_eligible(self):
        self.assertEqual(run(), ("pack", "resourcepack", ["1.1.0"]))
        # Newest first or last: the entries on main do both.
        self.assertEqual(run(head=TOP + OLD + NEW)[2], ["1.1.0"])
        self.assertEqual(run(head=TOP + NEW + NEW.replace("1.1.0", "1.2.0") + OLD)[2], ["1.1.0", "1.2.0"])

    def test_a_new_version_below_one_of_the_same_day_waits_for_a_reviewer(self):
        day = lambda v, d: v.replace("\nfile", f'\npublished = "{d}"\nfile')  # noqa: E731
        old, new = day(OLD, "2026-11-19"), day(NEW, "2026-11-19")
        self.assertEqual(run(base=TOP + old, head=TOP + new + old)[2], ["1.1.0"])
        self.assertEqual(run(base=TOP + old, head=TOP + old + day(NEW, "2026-11-20"))[2], ["1.1.0"])
        with self.assertRaisesRegex(ou.Refused, "below a version of the same day 2026-11-19"):
            run(base=TOP + old, head=TOP + old + new)
        for kind, ext in (("shader", ".hshader"), ("datapack", ".zip")):
            with self.subTest(kind=kind):
                top = TOP.replace("resourcepack", kind)
                old, new = OLD.replace(".zip", ext), NEW.replace(".zip", ext)
                self.assertEqual(run(base=top + old, head=top + new + old)[1], kind)

    def test_a_signed_entry_needs_signed_versions(self):
        top = TOP + 'key = "RWQ"\n'
        signed = NEW + 'signature = "untrusted comment: x"\n'
        self.assertEqual(run(base=top + OLD, head=top + signed + OLD)[0], "pack")
        with self.assertRaisesRegex(ou.Refused, "no signature"):
            run(base=top + OLD, head=top + NEW + OLD)

    def test_refused(self):
        cases = {
            "not in .* owner_ids": dict(author=7),
            "id True is not|not in": dict(author=True),
            "has no owner_ids": dict(base=TOP.replace("owner_ids = [42]\n", "") + OLD,
                                     head=TOP.replace("owner_ids = [42]\n", "") + NEW + OLD),
            "not in .*owner_ids.*": dict(base=TOP.replace("[42]", "[42.0]") + OLD,
                                         head=TOP.replace("[42]", "[42.0]") + NEW + OLD),
            "kind is 'native-mod'": dict(base=TOP.replace("resourcepack", "native-mod") + OLD,
                                         head=TOP.replace("resourcepack", "native-mod") + NEW + OLD),
            "kind is 'mod'": dict(base=TOP.replace("resourcepack", "mod") + OLD,
                                  head=TOP.replace("resourcepack", "mod") + NEW + OLD),
            "changes more than one": dict(changes=(("M", PATH), ("M", "README.md"))),
            "more than one modified": dict(changes=(("A", PATH),)),
            "more than one modified ": dict(changes=(("M", "archive/api-0.1/mods/pack.toml"),)),
            "changes more than its versions: kind": dict(head=TOP.replace("resourcepack", "shader") + NEW + OLD),
            "changes more than its versions: name": dict(head=TOP.replace('"Pack"', '"Pack 2"') + NEW + OLD),
            "changes more than its versions: owner_ids": dict(head=TOP.replace("[42]", "[42, 7]") + NEW + OLD),
            "changes more than its versions: key": dict(head=TOP + 'key = "RWQ"\n' + NEW + OLD),
            "only add versions": dict(head=TOP + NEW + OLD.replace("1.0.0.zip", "1.0.1.zip")),
            "only add versions ": dict(head=TOP + NEW),
            "only add versions  ": dict(head=TOP + OLD),
            "only add versions   ": dict(base=TOP + OLD + NEW.replace("1.1.0", "0.9.0"),
                                         head=TOP + NEW.replace("1.1.0", "0.9.0") + NEW + OLD),
            "changed on main": dict(main=TOP + NEW + OLD),
            "file is not a .zip": dict(head=TOP + NEW.replace("1.1.0.zip", "1.1.0.hmod") + OLD),
            "file is not a .hshader": dict(base=TOP.replace("resourcepack", "shader") + OLD,
                                           head=TOP.replace("resourcepack", "shader") + NEW + OLD),
            "in revoked.toml": dict(revoked=REVOKED + REVOKED.replace('"other"', '"pack"')),
            "doesn't parse": dict(head="id = "),
            "has id": dict(base=TOP.replace('"pack"', '"other"') + OLD, head=TOP.replace('"pack"', '"other"') + NEW),
        }
        for why, args in cases.items():
            with self.subTest(why=why), self.assertRaisesRegex(ou.Refused, why.strip()):
                run(**args)

    def test_an_archived_id_reactivation_waits_for_a_maintainer(self):
        # A new mods/<id>.toml for an id under archive/: its check's owner step stays red until a maintainer confirms
        # the owner by hand, so it never merges by itself, even from an owner adding only versions.
        for changes in ((("A", PATH),), (("A", PATH), ("M", "archive/native-v1/mods/pack.toml")),
                        (("A", PATH), ("D", "archive/api-0.1/mods/pack.toml"))):
            with self.subTest(changes=changes), self.assertRaisesRegex(ou.Refused, "more than one modified"):
                run(base="", main="", head=TOP + NEW + OLD, changes=changes)

    def test_same_is_strict_about_types(self):
        self.assertTrue(ou.same({"a": [1, "x"]}, {"a": [1, "x"]}))
        for a, b in ((1, 1.0), (1, True), ([1], (1,)), ({"a": 1}, {"a": 1, "b": 2})):
            with self.subTest(a=a, b=b):
                self.assertFalse(ou.same(a, b))


class PullRequest(unittest.TestCase):
    PULLS = [
        {"number": 1, "sha": "1" * 40, "repo": "someone/horizon-mods", "ref": "update"},
        {"number": 2, "sha": "2" * 40, "repo": "other/horizon-mods", "ref": "update"},
        {"number": 3, "sha": "3" * 40, "repo": None, "ref": "gone"},
    ]

    def test_found_by_head_repository_and_branch(self):
        self.assertEqual(ou.pick(self.PULLS, "1" * 40, "someone/horizon-mods", "update"), 1)
        for args in (("1" * 40, "other/horizon-mods", "update"), ("1" * 40, "someone/horizon-mods", "main"),
                     ("4" * 40, "someone/horizon-mods", "update")):
            with self.subTest(args=args), self.assertRaisesRegex(ou.Refused, "0 open pull requests"):
                ou.pick(self.PULLS, *args)
        with self.assertRaisesRegex(ou.Refused, "2 open pull requests"):
            ou.pick(self.PULLS + self.PULLS[:1], "1" * 40, "someone/horizon-mods", "update")

    def test_the_newest_check_run_counts(self):
        actions = {"slug": "github-actions"}
        ok = {"id": 1, "name": "check", "app": actions, "status": "completed", "conclusion": "success"}
        failed = {**ok, "id": 2, "conclusion": "failure"}
        self.assertTrue(ou.check_passed([ok]))
        self.assertFalse(ou.check_passed([ok, failed]))
        self.assertTrue(ou.check_passed([failed, {**ok, "id": 3}]))
        self.assertFalse(ou.check_passed([{**ok, "app": {"slug": "someone-else"}}]))
        self.assertFalse(ou.check_passed([{**ok, "name": "checks"}]))
        self.assertFalse(ou.check_passed([]))


class Main(unittest.TestCase):
    def test_a_refusal_sets_no_output_whatever_it_prints(self):
        # A file name can hold a newline: its line in the log must not be a workflow command that sets `pr`.
        def decide(env):
            raise ou.Refused("it changes more than one modified mods/<id>.toml: A x\n::set-output name=pr::1")

        with tempfile.TemporaryDirectory() as temp:
            output, summary = pathlib.Path(temp, "output"), pathlib.Path(temp, "summary")
            env = {"GITHUB_OUTPUT": str(output), "GITHUB_STEP_SUMMARY": str(summary)}
            with mock.patch.dict(ou.os.environ, env), mock.patch.object(ou, "decide", decide), \
                    contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(ou.main(), 0)
            self.assertFalse(output.exists())
        first, *rest = out.getvalue().splitlines()
        self.assertRegex(first, r"^::stop-commands::[0-9a-f]{32}$")
        # The line is printed, after commands were stopped, and the token that would resume them never is.
        self.assertTrue(any(line.startswith("::set-output name=pr::1") for line in rest))
        self.assertNotIn(first.split("::")[2], "\n".join(rest))


if __name__ == "__main__":
    unittest.main()
