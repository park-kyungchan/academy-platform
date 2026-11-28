"""Tests of the shared and print rules of specs/studyq-linux/core.md. Synthetic fixtures with invented values only.

The pure-Python tests need neither assets nor PyMuPDF (R26). The tests that draw a PDF run when
CHECK_STUDYQ_PYMUPDF=1 is set or `pymupdf` imports (R27); they are in test_print_zip.py.
"""

import hashlib
import io
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
MODULE = os.path.dirname(HERE)
ROOT = os.path.dirname(os.path.dirname(MODULE))
sys.path.insert(0, os.path.join(MODULE, "src"))

from studyq_linux import cli, manifest  # noqa: E402
from studyq_linux.errors import StudyqError  # noqa: E402

FONT_PATH = os.path.join(ROOT, "apps/academy/dc-gang-a/src/academy/web/assets/NanumGothic-Regular.ttf")

try:
    import pymupdf  # noqa: E402

    HAVE_PYMUPDF = True
except ImportError:
    pymupdf = None
    HAVE_PYMUPDF = False

RENDER_ENABLED = os.environ.get("CHECK_STUDYQ_PYMUPDF") == "1" or HAVE_PYMUPDF
if not RENDER_ENABLED:
    sys.stderr.write("skip: studyq_linux rendering tests (no pymupdf; set CHECK_STUDYQ_PYMUPDF=1 to require it)\n")
needs_pymupdf = unittest.skipUnless(RENDER_ENABLED, "pymupdf not available")


def run_cli(argv, stdin=b""):
    status, line = cli.run(argv, io.BytesIO(stdin))
    return status, line


def fake_manifest(files, **override):
    doc = {
        "schemaVersion": manifest.SCHEMA_VERSION,
        "reportZipSha256": manifest.PINNED_REPORT_ZIP_SHA256,
        "fontSha256": manifest.PINNED_FONT_SHA256,
        "fileCount": len(files),
        "files": files,
    }
    doc.update(override)
    return doc


def write_assets(directory, contents, extra_manifest=None, mutate=None):
    """Write `contents` (relative path -> bytes) and a matching manifest under `directory`."""
    files = {}
    for rel, data in contents.items():
        path = os.path.join(directory, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as handle:
            handle.write(data)
        files[rel] = hashlib.sha256(data).hexdigest()
    doc = fake_manifest(files, **(extra_manifest or {}))
    if mutate:
        mutate(doc)
    with open(os.path.join(directory, "manifest.json"), "w") as handle:
        json.dump(doc, handle)
    return files


def font_bytes():
    with open(FONT_PATH, "rb") as handle:
        return handle.read()


class TempDirCase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix="studyq-test-")
        self.addCleanup(shutil.rmtree, self.tmp, True)

    def path(self, *parts):
        return os.path.join(self.tmp, *parts)


class WordsAndInput(TempDirCase):
    def test_r3_usage_invalid_before_stdin_is_read(self):
        class Poison:
            def read(self, _n=-1):
                raise AssertionError("stdin read")

        cases = [
            [],
            ["bogus"],
            ["render", "--type", "course_book", "--assets", "a", "--out", "o"],
            ["assemble-print"],
            ["assemble-print", "--out"],
            ["assemble-print", "--out", "o", "--assets", "a"],
            ["assemble-print", "--out", "o", "--out", "p"],
            ["assemble-print", "--out", "o", "positional"],
            ["assemble-print", "--out=o"],
            ["assemble-print", "--out", "--other"],
        ]
        for argv in cases:
            status, line = cli.run(argv, Poison())
            doc = json.loads(line)
            self.assertNotEqual(status, 0, argv)
            self.assertEqual(doc["code"], "usage_invalid", argv)
            self.assertEqual(set(doc), {"status", "code", "detail"})

    def refuse(self, stdin):
        status, line = run_cli(["assemble-print", "--out", self.path("o.pdf")], stdin)
        doc = json.loads(line)
        self.assertNotEqual(status, 0)
        self.assertEqual(doc["code"], "input_invalid", line)
        self.assertFalse(os.path.exists(self.path("o.pdf")))
        return doc["detail"]

    def test_r4_input_invalid(self):
        self.refuse(b"\xff\xfe{}")
        self.refuse(b"")
        self.refuse(b"[]")
        self.refuse(b"{} {}")
        self.refuse(b'{"a":1,"a":2}')
        self.refuse(b'{"kind":NaN}')
        self.refuse(b" " * (4 * 1024 * 1024 + 1))
        self.refuse(b"{}")
        self.refuse(json.dumps({"archive_root": self.tmp, "kind": "exam", "items": [], "surplus": 1}).encode())
        self.refuse(json.dumps({"archive_root": self.tmp, "kind": "exam", "items": "x"}).encode())

    def test_r7_details_hold_no_input_value(self):
        secret = "Zq" + "Xv" + "Student-Name"
        for doc in (
            {"archive_root": self.tmp, "kind": "exam", "items": [{"lecture_key": "oops-" + secret, "sel_type": "obj"}]},
            {"archive_root": self.tmp, "kind": "exam", "items": [], "surplus-" + secret: 1},
            {"archive_root": "/" + secret, "kind": "examtext", "items": [{"examtext_key": 1, "sel_type": "obj"}]},
        ):
            status, line = run_cli(["assemble-print", "--out", self.path("o.pdf")], json.dumps(doc).encode())
            self.assertNotEqual(status, 0)
            self.assertNotIn(secret, line)
            self.assertLessEqual(len(json.loads(line)["detail"].encode()), 256)
        self.assertLessEqual(len(StudyqError("render_failed", "é" * 400).detail.encode()), 256)

    def test_r7_unexpected_failure_reports_class_only(self):
        class Boom:
            def read(self, _n=-1):
                raise RuntimeError("secret-student-value")

        status, line = cli.run(["assemble-print", "--out", self.path("o.pdf")], Boom())
        doc = json.loads(line)
        self.assertEqual((doc["code"], doc["detail"]), ("render_failed", "RuntimeError"))

    def test_r2_error_line_is_one_line_with_a_known_code(self):
        status, line = run_cli(["assemble-print", "--out", self.path("o.pdf")], b"[]")
        self.assertEqual(line.count("\n"), 0)
        self.assertEqual(list(json.loads(line)), ["status", "code", "detail"])
        self.assertIn(json.loads(line)["code"], __import__("studyq_linux.errors", fromlist=["CODES"]).CODES)

    def test_entry_point_is_python_dash_m(self):
        env = dict(os.environ, PYTHONPATH=os.path.join(MODULE, "src"))
        proc = subprocess.run([sys.executable, "-B", "-m", "studyq_linux", "bogus"], env=env, capture_output=True, text=True, cwd=self.tmp)
        self.assertEqual(proc.returncode, 2)
        self.assertEqual(json.loads(proc.stdout)["code"], "usage_invalid")
        self.assertEqual(proc.stdout.count("\n"), 1)
        self.assertEqual(proc.stderr, "")


class AtomicWrite(TempDirCase):
    def test_r6_written_beside_and_renamed_never_partial(self):
        out = self.path("sub", "r.pdf")
        os.makedirs(os.path.dirname(out))
        cli.write_atomic(out, b"%PDF-1.4 invented")
        self.assertEqual(os.listdir(os.path.dirname(out)), ["r.pdf"])
        with self.assertRaises(StudyqError) as caught:
            cli.write_atomic(out, b"%PDF-1.4 other")
        self.assertEqual(caught.exception.code, "output_exists")
        self.assertEqual(os.listdir(os.path.dirname(out)), ["r.pdf"])
        with open(out, "rb") as handle:
            self.assertEqual(handle.read(), b"%PDF-1.4 invented")

    def test_r6_temp_file_removed_when_the_rename_loses_a_race(self):
        out = self.path("race.pdf")
        original = os.link

        def late_file(src, dst, *a, **k):
            with open(dst, "wb") as handle:
                handle.write(b"winner")
            return original(src, dst, *a, **k)

        os.link = late_file
        try:
            with self.assertRaises(StudyqError) as caught:
                cli.write_atomic(out, b"%PDF-1.4 loser")
        finally:
            os.link = original
        self.assertEqual(caught.exception.code, "output_exists")
        self.assertEqual(sorted(os.listdir(self.tmp)), ["race.pdf"])
        with open(out, "rb") as handle:
            self.assertEqual(handle.read(), b"winner")


class AssetManifest(TempDirCase):
    def verify(self, contents, **kw):
        write_assets(self.tmp, contents, **kw)
        return manifest.verify_assets_dir(self.tmp)

    def code(self, contents, **kw):
        with self.assertRaises(StudyqError) as caught:
            self.verify(contents, **kw)
        return caught.exception.code

    def base(self):
        return {manifest.FONT_REL: font_bytes(), "image/mark_o.jpg": b"not really a picture"}

    def test_r9_good_manifest_gives_verified_bundle(self):
        bundle = self.verify(self.base())
        self.assertEqual(bundle.read("image/mark_o.jpg"), b"not really a picture")
        self.assertTrue(bundle.has(manifest.FONT_REL))
        with self.assertRaises(StudyqError):
            bundle.read("image/other.jpg")

    def test_r9_manifest_absent_or_malformed(self):
        with self.assertRaises(StudyqError) as caught:
            manifest.verify_assets_dir(self.tmp)
        self.assertEqual(caught.exception.code, "asset_manifest_invalid")
        for raw in (b"[]", b"not json", b"{}", b'{"schemaVersion":"studyq-assets/1"}'):
            with open(self.path("manifest.json"), "wb") as handle:
                handle.write(raw)
            with self.assertRaises(StudyqError) as caught:
                manifest.verify_assets_dir(self.tmp)
            self.assertEqual(caught.exception.code, "asset_manifest_invalid")

    def test_r9_schema_members_and_consistency(self):
        self.assertEqual(self.code(self.base(), extra_manifest={"schemaVersion": "studyq-assets/2"}), "asset_manifest_invalid")
        self.assertEqual(self.code(self.base(), mutate=lambda d: d.update(extra=1)), "asset_manifest_invalid")
        self.assertEqual(self.code(self.base(), mutate=lambda d: d.pop("fileCount")), "asset_manifest_invalid")
        self.assertEqual(self.code(self.base(), extra_manifest={"fileCount": 99}), "asset_manifest_invalid")
        self.assertEqual(self.code(self.base(), extra_manifest={"fileCount": True}), "asset_manifest_invalid")
        self.assertEqual(self.code(self.base(), mutate=lambda d: d["files"].update({"image/x.jpg": "zz"})), "asset_manifest_invalid")
        self.assertEqual(self.code(self.base(), extra_manifest={"reportZipSha256": "ABC"}), "asset_manifest_invalid")

    def test_r9_unsafe_paths_refused(self):
        for rel in ("/etc/passwd", "image/../x.jpg", "image//x.jpg", "../image/x.jpg", "image/./x.jpg", "other/x.jpg", "image\\x.jpg", "report.zip", "image"):
            files = {manifest.FONT_REL: manifest.PINNED_FONT_SHA256, rel: "0" * 64}
            doc = fake_manifest(files)
            with open(self.path("manifest.json"), "w") as handle:
                json.dump(doc, handle)
            with self.assertRaises(StudyqError) as caught:
                manifest.verify_assets_dir(self.tmp)
            self.assertEqual(caught.exception.code, "asset_manifest_invalid", rel)

    def test_r9_pins_and_hash_mismatch(self):
        zero = "0" * 64
        self.assertEqual(self.code(self.base(), extra_manifest={"reportZipSha256": zero}), "asset_hash_mismatch")
        self.assertEqual(self.code(self.base(), extra_manifest={"fontSha256": zero}), "asset_hash_mismatch")
        self.assertEqual(self.code(self.base(), mutate=lambda d: d["files"].update({"image/mark_o.jpg": zero})), "asset_hash_mismatch")
        self.assertEqual(self.code(self.base(), mutate=lambda d: d["files"].update({manifest.FONT_REL: zero})), "asset_hash_mismatch")
        self.assertEqual(self.code({manifest.FONT_REL: b"another font"}), "asset_hash_mismatch")

    def test_r9_missing_file_and_missing_font_entry(self):
        def drop(doc):
            doc["files"]["image/gone.jpg"] = "1" * 64
            doc["fileCount"] = len(doc["files"])

        self.assertEqual(self.code(self.base(), mutate=drop), "asset_missing")
        self.assertEqual(self.code({"image/mark_o.jpg": b"x"}, extra_manifest={}), "asset_manifest_invalid")

    def test_r9_symlink_is_not_followed(self):
        write_assets(self.tmp, self.base())
        target = self.path("image", "mark_o.jpg")
        os.unlink(target)
        os.symlink(self.path("manifest.json"), target)
        with self.assertRaises(StudyqError) as caught:
            manifest.verify_assets_dir(self.tmp)
        self.assertEqual(caught.exception.code, "asset_missing")

    def test_r9_file_changed_after_verification_is_refused_on_read(self):
        bundle = self.verify(self.base())
        with open(self.path("image", "mark_o.jpg"), "wb") as handle:
            handle.write(b"changed afterwards")
        with self.assertRaises(StudyqError) as caught:
            bundle.read("image/mark_o.jpg")
        self.assertEqual(caught.exception.code, "asset_hash_mismatch")

    def test_r9_repo_font_matches_the_pins(self):
        self.assertEqual(hashlib.sha256(font_bytes()).hexdigest(), manifest.PINNED_FONT_SHA256)

    def test_r10_assets_are_untouched_and_the_network_is_not_used(self):
        write_assets(self.tmp, self.base())

        def snapshot():
            out = {}
            for base, _dirs, names in os.walk(self.tmp):
                for name in names:
                    p = os.path.join(base, name)
                    st = os.stat(p)
                    out[p] = (st.st_mtime_ns, st.st_size, st.st_mode)
            return out, sorted(os.listdir(self.tmp))

        before = snapshot()
        original = socket.socket.connect

        def no_network(*_a, **_k):
            raise AssertionError("network used")

        socket.socket.connect = no_network
        try:
            manifest.verify_assets_dir(self.tmp)
            doc = {"archive_root": self.tmp, "kind": "exam", "items": [{"lecture_key": 5, "sel_type": "obj"}]}
            status, line = run_cli(["assemble-print", "--out", self.path("elsewhere.pdf")], json.dumps(doc).encode())
            self.assertNotEqual(status, 0, line)
        finally:
            socket.socket.connect = original
        self.assertEqual(before, snapshot())


class Pure(unittest.TestCase):
    def test_r26_image_dimensions_are_read_from_headers(self):
        import struct

        from studyq_linux.imageinfo import image_dimensions

        png = b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", 44, 36) + b"\x08\x06\x00\x00\x00"
        self.assertEqual(image_dimensions(png), (44, 36))
        jpeg = b"\xff\xd8\xff\xe0" + struct.pack(">H", 4) + b"JF" + b"\xff\xc2" + struct.pack(">HBHH", 11, 8, 1017, 2198) + b"\x00" * 6
        self.assertEqual(image_dimensions(jpeg), (2198, 1017))
        self.assertIsNone(image_dimensions(b"GIF89a"))
        self.assertIsNone(image_dimensions(b"\xff\xd8\xff"))

class PrivacyCheck(TempDirCase):
    """I6 (R28): the patterns live only in `check`; positive fixtures are built here from split strings."""

    def scan(self, *dirs, roster=None):
        env = dict(os.environ, CHECK_I6_ROOTS=":".join(dirs))
        env.pop("CHECK_STUDYQ_ROSTER", None)
        if roster:
            env["CHECK_STUDYQ_ROSTER"] = roster
        return subprocess.run(["bash", os.path.join(ROOT, "check")], env=env, capture_output=True, text=True)

    def test_r28_check_passes_on_the_current_specs_and_module(self):
        proc = self.scan(os.path.join(ROOT, "specs"), MODULE)
        self.assertEqual((proc.returncode, proc.stdout.strip()), (0, "i6: ok"), proc.stderr)

    def test_r28_check_fails_on_secret_values(self):
        fixtures = {
            "key.txt": "-----BEGIN " + "RSA PRIVATE " + "KEY-----\nabc\n",
            "aws.txt": "id = " + "AKIA" + "ABCDEFGHIJKLMNOP\n",
            "gh.txt": "t " + "ghp_" + "a" * 36 + "\n",
            "sk.txt": "k " + "sk-" + "b" * 24 + "\n",
            "assign.json": '{"pass' + 'word": "hunter2' + 'hunter2"}\n',
            "archive.py": "archive_" + "key = '" + "0123456789abcdef" + "'\n",
            "bearer.txt": "Authorization: " + "Bearer " + "c" * 30 + "\n",
        }
        for name, text in fixtures.items():
            directory = os.path.join(self.tmp, name.replace(".", "_"))
            os.makedirs(directory)
            with open(os.path.join(directory, name), "w") as handle:
                handle.write(text)
            proc = self.scan(directory)
            self.assertEqual(proc.returncode, 5, name)
            self.assertIn(name, proc.stderr)
            self.assertNotIn("hunter2", proc.stderr + proc.stdout)
            self.assertNotIn("ABCDEFGHIJKLMNOP", proc.stderr + proc.stdout)

    def test_r28_check_passes_on_plain_text_and_the_two_pinned_hashes_and_skips_private_trees(self):
        directory = os.path.join(self.tmp, "clean")
        os.makedirs(os.path.join(directory, "private"))
        with open(os.path.join(directory, "a.md"), "w") as handle:
            handle.write("hash %s\nfont %s\nthe archive key is host configuration\n" % (manifest.PINNED_REPORT_ZIP_SHA256, manifest.PINNED_FONT_SHA256))
        with open(os.path.join(directory, "private", "b.txt"), "w") as handle:
            handle.write("password" + ' = "' + "x" * 12 + '"\n')
        proc = self.scan(directory)
        self.assertEqual(proc.returncode, 0, proc.stderr)

    def test_r28_roster_names_fail_the_check_without_being_printed(self):
        name = "Invented" + "Pupil" + "Name"
        directory = os.path.join(self.tmp, "fixture")
        os.makedirs(directory)
        with open(os.path.join(directory, "f.json"), "w") as handle:
            handle.write('{"student": "%s"}\n' % name)
        roster = os.path.join(self.tmp, "roster.txt")
        with open(roster, "w") as handle:
            handle.write("\n" + name + "\n\n")
        proc = self.scan(directory, roster=roster)
        self.assertEqual(proc.returncode, 5)
        self.assertIn("f.json", proc.stderr)
        self.assertNotIn(name, proc.stderr + proc.stdout)
        self.assertEqual(self.scan(directory).returncode, 0)

    def test_r28_check_wiring(self):
        with open(os.path.join(ROOT, "check")) as handle:
            text = handle.read()
        self.assertIn("modules/studyq-linux/tests", text)
        self.assertIn("CHECK_STUDYQ_PYMUPDF", text)
        self.assertIn("i6_scan specs modules/studyq-linux", text)


class Gate(unittest.TestCase):
    def test_r26_pure_tests_do_not_import_pymupdf_for_planning(self):
        code = (
            "import sys; sys.path.insert(0, %r); sys.modules['pymupdf'] = None; "
            "from studyq_linux import cli, errors, imageinfo, manifest, print_zip; print('ok')"
        ) % os.path.join(MODULE, "src")
        proc = subprocess.run([sys.executable, "-B", "-I", "-c", code], capture_output=True, text=True)
        self.assertEqual((proc.returncode, proc.stdout.strip()), (0, "ok"), proc.stderr)

    def test_r27_rendering_without_pymupdf_refuses_with_render_failed(self):
        code = (
            "import sys, io, json; sys.path.insert(0, %r); sys.modules['pymupdf'] = None; "
            "from studyq_linux import print_zip; "
            "\ntry:\n print_zip.load_pymupdf()\nexcept Exception as e:\n print(e.code, e.detail)\n"
        ) % os.path.join(MODULE, "src")
        proc = subprocess.run([sys.executable, "-B", "-I", "-c", code], capture_output=True, text=True)
        self.assertEqual(proc.stdout.strip(), "render_failed pymupdf_unavailable", proc.stderr)

    def test_r27_gate_condition_is_documented_in_the_module(self):
        self.assertEqual(RENDER_ENABLED, os.environ.get("CHECK_STUDYQ_PYMUPDF") == "1" or HAVE_PYMUPDF)

    def test_retired_report_rules_are_marked_in_the_specification(self):
        with open(os.path.join(ROOT, "specs/studyq-linux/core.md")) as handle:
            text = handle.read()
        for n in (5, 12, 13, 14, 15, 16, 17, 18, 19, 29):
            self.assertIn("- R%d: (removed)" % n, text)
        for name in ("course_book.py", "parity.py", "render.py", "renderers", "schema.py", "textutil.py", "geometry.py"):
            self.assertFalse(os.path.exists(os.path.join(MODULE, "src", "studyq_linux", name)), name)


if __name__ == "__main__":
    unittest.main()
