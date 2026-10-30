"""python3 scripts/test_owner_update.py"""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(__file__))
import owner_update as ou  # noqa: E402

KEY = "a test update key"
SHA = "ab" * 32
FIELDS = {"id": "zoom", "version": "1.1.0", "sha256": SHA, "file": "zoom-1.1.0.hnmod", "submitter": "alice"}


def entry(owners='["alice"]', versions=("1.0.0",), extra="", key='key = "RWK"\n'):
    head = f'id = "zoom"\nkind = "native-mod"\nname = "Zoom"\nsummary = "s"\nowners = {owners}\nlicense = "MIT"\nsource = "https://x"\n{key}{extra}\n'
    return head + "".join(
        f'[[versions]]\nversion = "{v}"\npublished = "2026-10-30"\ngame = "26.3"\nfile = "zoom-{v}.hnmod"\n'
        f'sha256 = "{SHA if v == "1.1.0" else "00" * 32}"\nsignature = "sig"\n\n'
        for v in versions
    )


BASE = entry()
GOOD = entry(versions=("1.1.0", "1.0.0"))
FILES = [{"filename": "mods/zoom.toml", "status": "modified"}]


def run(head=GOOD, files=FILES, main=BASE, base=BASE, revoked="", fields=FIELDS):
    return ou.check_diff(fields, files, base, head, main, revoked)


class Trailer(unittest.TestCase):
    def body(self, fields=FIELDS, key=KEY):
        lines = "\n".join(f"{k}: {v}" for k, v in fields.items())
        return f"hello\n\n<!-- owner-update\n{lines}\nhmac: {ou.mac(key, fields)}\n-->"

    def test_valid(self):
        f = ou.trailer(self.body())
        ou.verify_trailer(KEY, f)
        self.assertEqual(f["submitter"], "alice")

    def test_matches_the_form(self):
        # crates/horizon-mod-registry/tests/submit.rs builds the same message.
        f = {"id": "coords_hud", "version": "0.1.0", "sha256": "e1" * 32, "file": "coords_hud-0.1.0.hmod", "submitter": "alice"}
        import hashlib, hmac
        msg = "owner-update-v1\ncoords_hud\n0.1.0\n" + "e1" * 32 + "\ncoords_hud-0.1.0.hmod\nalice"
        self.assertEqual(ou.mac(KEY, f), hmac.new(KEY.encode(), msg.encode(), hashlib.sha256).hexdigest())

    def test_none(self):
        self.assertIsNone(ou.trailer("no trailer"))
        self.assertIsNone(ou.trailer(None))

    def test_forged(self):
        for body in (self.body(key="another"), self.body().replace("alice", "mallory")):
            with self.assertRaises(ou.Refused):
                ou.verify_trailer(KEY, ou.trailer(body))
        with self.assertRaises(ou.Refused):
            ou.verify_trailer("", ou.trailer(self.body()))

    def test_malformed(self):
        with self.assertRaises(ou.Refused):
            ou.trailer(self.body().replace("file:", "other:"))
        with self.assertRaises(ou.Refused):
            ou.trailer(self.body().replace("-->", "id: x\n-->"))


class Diff(unittest.TestCase):
    def refused(self, part, **kw):
        with self.assertRaises(ou.Refused) as e:
            run(**kw)
        self.assertIn(part, str(e.exception))

    def test_an_owners_new_version(self):
        self.assertEqual(run(), "native-mod")
        self.assertEqual(run(head=entry(versions=("1.0.0", "1.1.0"))), "native-mod")  # appended, as the diff reads

    def test_files(self):
        self.refused("exactly", files=FILES + [{"filename": "icons/zoom.png", "status": "added"}])
        self.refused("exactly", files=[{"filename": "mods/zoom.toml", "status": "added"}])
        self.refused("exactly", files=[{"filename": "mods/other.toml", "status": "modified"}])

    def test_owners_and_revoked(self):
        self.refused("not in", base=entry('["bob"]'), main=entry('["bob"]'), head=entry('["bob"]', ("1.1.0", "1.0.0")))
        self.refused("revoked", revoked='[[revoked]]\nid = "zoom"\nreason = "r"\ndate = "2026-10-30"\n')
        self.refused("changed on main", main=entry(extra='description = "new"\n'))

    def test_only_a_version(self):
        self.refused("owners", head=entry('["alice", "mallory"]', ("1.1.0", "1.0.0")))
        self.refused("key", head=entry(versions=("1.1.0", "1.0.0"), key='key = "RWOther"\n'))
        self.refused("summary", head=GOOD.replace('summary = "s"', 'summary = "z"'))
        self.refused("icon", head=entry(versions=("1.1.0", "1.0.0"), extra='icon = "https://x/i.png"\n'))

    def test_the_version_is_the_signed_one(self):
        self.refused("sha256", head=GOOD.replace(SHA, "cd" * 32))
        self.refused("exactly one", head=entry(versions=("1.1.0", "1.0.5", "1.0.0")))
        self.refused("exactly one", head=entry(versions=("1.1.0",)))
        self.refused("version", fields={**FIELDS, "version": "9.9.9"})
        self.refused("no signature", head=GOOD.replace('signature = "sig"\n', "", 1))

    def test_a_published_version_is_never_edited(self):
        self.refused("exactly one", head=entry(versions=("1.1.0", "1.0.0")).replace("2026-10-30", "2026-10-31", 2))


class Waiting(unittest.TestCase):
    def test_it(self):
        run_ = lambda name, status="completed", conclusion="success": {"name": name, "status": status, "conclusion": conclusion}  # noqa: E731
        self.assertEqual(ou.waiting([], "owner-update"), (False, None))
        self.assertEqual(ou.waiting([run_("owner-update", "in_progress", None)], "owner-update"), (False, None))
        self.assertEqual(ou.waiting([run_("check", "in_progress", None)], "owner-update"), (False, None))
        self.assertEqual(ou.waiting([run_("check")], "owner-update"), (True, None))
        self.assertEqual(ou.waiting([run_("check"), run_("native", "queued", None)], "owner-update"), (False, None))
        self.assertEqual(ou.waiting([run_("check"), run_("native", conclusion="skipped")], "owner-update"), (True, None))
        self.assertEqual(ou.waiting([run_("check", conclusion="failure")], "owner-update"), (True, "check ended failure"))
        self.assertEqual(ou.waiting([run_("check"), run_("native", conclusion="cancelled")], "owner-update")[1], "native ended cancelled")


if __name__ == "__main__":
    unittest.main()
