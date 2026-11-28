"""Tests of specs/studyq-linux/core.md. Synthetic fixtures with invented values only.

The pure-Python tests need neither assets nor PyMuPDF (R26). The rendering tests run when
CHECK_STUDYQ_PYMUPDF=1 is set or `pymupdf` imports (R27); then they build synthetic templates
with PyMuPDF and the repository's NanumGothic font.
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

from studyq_linux import course_book as cb  # noqa: E402
from studyq_linux import geometry, manifest, textutil  # noqa: E402
from studyq_linux import cli  # noqa: E402
from studyq_linux.errors import StudyqError  # noqa: E402
from studyq_linux.schema import validate_course_book  # noqa: E402

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
ASSETS_DIR = os.environ.get("STUDYQ_ASSETS_DIR")
if not ASSETS_DIR:
    sys.stderr.write("skip: studyq_linux parity against the official templates (no assets directory given)\n")

IMAGE_SIZES = {
    "image/mark_o.jpg": (75, 56),
    "image/mark_x.jpg": (75, 56),
    "image/course_book/1p/grade_03.jpg": (1040, 709),
    cb.SCORE_RANGE.rel: (2128, 261),
    cb.TEST_LIST.rel: (2198, 1017),
    cb.MUNIT.rel: (2198, 1017),
    cb.DOT: (36, 36),
    cb.STAR: (44, 44),
    cb.SQUARE: (36, 36),
}
for _n in range(1, 8):
    IMAGE_SIZES[cb.TEMPLATE_DIR + "coursebook_%d.jpg" % _n] = (248, 351)

FAKE_CTX = cb.PlanContext(lambda text, size: len(text) * size * 0.6, lambda rel: IMAGE_SIZES[rel])


def synthetic_input(chapters=3, questions=3):
    """A synthetic `course_book` input: invented words and numbers only."""
    return {
        "period": "2026.01.01 ~ 2026.03.31",
        "student": "Sample Learner",
        "grade": 3,
        "my_average": 82.5,
        "all_average": 70,
        "comment": "First sentence of the comment.\\r\\nSecond sentence, a return visit.\nThird line.\r\nFourth line.",
        "score_range": {"my": 82.5, "average": 70},
        "test_list": [
            {"no": i + 1, "title": "Sample test %d" % (i + 1), "questions": 20 + i, "average": 60 + i, "score": 70 + 2 * i, "date": "2026-02-%02d" % (i + 1)}
            for i in range(4)
        ],
        "bunit_list": [
            {
                "name": "Chapter %d" % (c + 1),
                "my_score": 60 + 10 * c,
                "average": 50 + 5 * c,
                "questions": [
                    {
                        "no": q + 1,
                        "type": "Kind %d-%d" % (c + 1, q + 1),
                        "question_type": "Objective" if q % 2 == 0 else "Subjective",
                        "difficulty": ["Top", "High", "Mid", "Low"][q % 4],
                        "correct_rate": (30 + 3 * q + c) % 100,
                        "correct": q % 2 == 0,
                    }
                    for q in range(questions)
                ],
            }
            for c in range(chapters)
        ],
        "difficulty": {
            "top": {"questions": 2, "score": 40},
            "high": {"questions": 3, "score": 55.5},
            "mid": {"questions": 4, "score": 70},
            "low": {"questions": 1, "score": 100},
        },
        "munit_list": [{"name": "Unit %d" % (i + 1), "correct": 5 + i, "total": 10, "average": 55 + i} for i in range(5)],
    }


def planned(data=None):
    return cb.plan_course_book(validate_course_book(data or synthetic_input()), FAKE_CTX)


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


def picture(width, height, fmt):
    pix = pymupdf.Pixmap(pymupdf.csRGB, pymupdf.IRect(0, 0, width, height), False)
    pix.clear_with(245)
    return pix.tobytes(fmt)


_CONTENTS = {}


def synthetic_contents(overflow=False):
    if overflow not in _CONTENTS:
        _CONTENTS[overflow] = _build_contents(overflow)
    return _CONTENTS[overflow]


def _build_contents(overflow):
    """Synthetic templates and pictures (real picture sizes, plain colours) plus the repo font."""
    contents = {manifest.FONT_REL: font_bytes()}
    for rel, (w, h) in IMAGE_SIZES.items():
        contents[rel] = picture(w, h, "png" if rel.endswith(".png") else "jpg")
    if overflow:
        contents[cb.TEMPLATE_DIR + "coursebook_5.jpg"] = picture(248, 351, "jpg")
    return contents


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
            ["render"],
            ["render", "--type", "course_book", "--assets", "a"],
            ["render", "--type", "course_book", "--type", "cism", "--assets", "a", "--out", "o"],
            ["render", "--type", "course_book", "--assets", "a", "--out", "o", "--extra", "x"],
            ["render", "--type", "course_book", "--assets", "a", "--out", "o", "positional"],
            ["render", "--type=course_book", "--assets", "a", "--out", "o"],
            ["render", "--type", "--assets", "a", "--out", "o"],
            ["assemble-print"],
            ["assemble-print", "--out", "o", "--assets", "a"],
        ]
        for argv in cases:
            status, line = cli.run(argv, Poison())
            doc = json.loads(line)
            self.assertNotEqual(status, 0, argv)
            self.assertEqual(doc["code"], "usage_invalid", argv)
            self.assertEqual(set(doc), {"status", "code", "detail"})

    def test_r5_type_unknown(self):
        status, line = run_cli(["render", "--type", "other", "--assets", self.tmp, "--out", self.path("o.pdf")], b"{}")
        self.assertEqual((status, json.loads(line)["code"]), (1, "type_unknown"))

    def refuse(self, stdin):
        status, line = run_cli(["render", "--type", "course_book", "--assets", self.tmp, "--out", self.path("o.pdf")], stdin)
        doc = json.loads(line)
        self.assertNotEqual(status, 0)
        self.assertEqual(doc["code"], "input_invalid", line)
        self.assertFalse(os.path.exists(self.path("o.pdf")))
        return doc["detail"]

    def test_r4_input_invalid(self):
        good = synthetic_input()
        self.refuse(b"\xff\xfe{}")
        self.refuse(b"")
        self.refuse(b"[]")
        self.refuse(b"{} {}")
        self.refuse(b'{"a":1,"a":2}')
        self.refuse(b'{"grade":NaN}')
        self.refuse(b" " * (4 * 1024 * 1024 + 1))
        missing = dict(good)
        del missing["comment"]
        self.assertEqual(self.refuse(json.dumps(missing).encode()), "missing:comment")
        extra = dict(good, surplus=1)
        self.refuse(json.dumps(extra).encode())
        wrong = dict(good, grade="3")
        self.assertEqual(self.refuse(json.dumps(wrong).encode()), "type:grade")
        wrong = dict(good, grade=True)
        self.refuse(json.dumps(wrong).encode())
        deep = json.loads(json.dumps(good))
        deep["bunit_list"][1]["questions"][0]["correct"] = 1
        self.assertEqual(self.refuse(json.dumps(deep).encode()), "type:bunit_list[1].questions[0].correct")
        deep = json.loads(json.dumps(good))
        deep["bunit_list"] = []
        self.refuse(json.dumps(deep).encode())
        deep = json.loads(json.dumps(good))
        deep["test_list"] = deep["test_list"] * 3
        self.refuse(json.dumps(deep).encode())

    def test_r7_details_hold_no_input_value(self):
        secret = "Zq" + "Xv" + "Student-Name"
        data = synthetic_input()
        data["student"] = secret
        data["bunit_list"][0]["questions"][0]["no"] = "oops-" + secret
        data["surplus-" + secret] = 1
        status, line = run_cli(["render", "--type", "course_book", "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(data).encode())
        self.assertNotIn(secret, line)
        self.assertLessEqual(len(json.loads(line)["detail"].encode()), 256)
        data = synthetic_input()
        data["student"] = secret
        status, line = run_cli(["render", "--type", "course_book", "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(data).encode())
        self.assertNotIn(secret, line)
        self.assertEqual(json.loads(line)["code"], "asset_manifest_invalid")
        self.assertLessEqual(len(StudyqError("render_failed", "é" * 400).detail.encode()), 256)

    def test_r7_unexpected_failure_reports_class_only(self):
        class Boom:
            def read(self, _n=-1):
                raise RuntimeError("secret-student-value")

        status, line = cli.run(["render", "--type", "course_book", "--assets", self.tmp, "--out", self.path("o.pdf")], Boom())
        doc = json.loads(line)
        self.assertEqual((doc["code"], doc["detail"]), ("render_failed", "RuntimeError"))

    def test_r2_error_line_is_one_line_with_a_known_code(self):
        status, line = run_cli(["render", "--type", "course_book", "--assets", self.tmp, "--out", self.path("o.pdf")], b"[]")
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

    def test_r9_refusal_through_the_command_writes_no_pdf(self):
        write_assets(self.tmp, self.base(), extra_manifest={"fontSha256": "0" * 64})
        out = self.path("o.pdf")
        status, line = run_cli(["render", "--type", "course_book", "--assets", self.tmp, "--out", out], json.dumps(synthetic_input()).encode())
        self.assertEqual((status, json.loads(line)["code"]), (1, "asset_hash_mismatch"))
        self.assertEqual([n for n in os.listdir(self.tmp) if n.endswith(".pdf") or n.endswith(".tmp")], [])

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
            run_cli(["render", "--type", "course_book", "--assets", self.tmp, "--out", self.path("elsewhere.pdf")], json.dumps(synthetic_input()).encode())
        finally:
            socket.socket.connect = original
        after = snapshot()
        after[0].pop(self.path("elsewhere.pdf"), None)
        self.assertEqual(before, (after[0], [n for n in after[1] if n != "elsewhere.pdf"]))


class Layout(unittest.TestCase):
    def test_r29_template_sequence(self):
        plan = planned()
        self.assertEqual(
            [p.template.rsplit("/", 1)[1] for p in plan.pages],
            ["coursebook_1.jpg", "coursebook_2.jpg", "coursebook_3.jpg"] + ["coursebook_4.jpg"] * 3 + ["coursebook_6.jpg", "coursebook_7.jpg"],
        )
        self.assertEqual(len(plan.pages), 8)

    def test_r29_overflow_uses_template_5_and_page_count_follows_chapters(self):
        self.assertEqual(len(planned(synthetic_input(chapters=1)).pages), 6)
        self.assertEqual(len(planned(synthetic_input(chapters=5)).pages), 10)
        plan = planned(synthetic_input(chapters=2, questions=26))
        names = [p.template.rsplit("/", 1)[1] for p in plan.pages]
        self.assertEqual(names[3:7], ["coursebook_4.jpg", "coursebook_5.jpg", "coursebook_4.jpg", "coursebook_5.jpg"])
        self.assertEqual(names[-2:], ["coursebook_6.jpg", "coursebook_7.jpg"])
        self.assertEqual(cb.template_sequence([25, 26]).count(cb.TEMPLATE_DIR + "coursebook_5.jpg"), 1)
        self.assertIn(cb.TEMPLATE_DIR + "coursebook_5.jpg", cb.required_assets(validate_course_book(synthetic_input(2, 26))))
        self.assertNotIn(cb.TEMPLATE_DIR + "coursebook_5.jpg", cb.required_assets(validate_course_book(synthetic_input())))

    def test_r13_removed_rule_is_marked_in_the_specification(self):
        with open(os.path.join(ROOT, "specs/studyq-linux/core.md")) as handle:
            spec = handle.read()
        self.assertTrue(("- R13: (removed)\n" in spec), "R13 is not marked (removed)")
        self.assertIn("- R29: ", spec)

    def test_r14_page_4_values_sit_in_the_column_of_their_heading(self):
        data = synthetic_input()
        plan = planned(data)
        page = plan.pages[3]
        by_id = {f.id: f for f in page.fields}
        q = data["bunit_list"][0]["questions"][1]
        expected = {
            "no": str(q["no"]),
            "type": q["type"],
            "question_type": q["question_type"],
            "difficulty": q["difficulty"],
            "correct_rate": textutil.format_number(q["correct_rate"]),
        }
        for name, text in expected.items():
            f = by_id["p4.c0.row1.%s" % name]
            self.assertEqual(f.text, text)
            x0, x1 = cb.CHAPTER_COLUMNS_PX[name]
            self.assertEqual((f.box[0], f.box[2]), (geometry.px(x0), geometry.px(x1)))
        columns = sorted(cb.CHAPTER_COLUMNS_PX.items(), key=lambda kv: kv[1][0])
        self.assertEqual([n for n, _ in columns], ["no", "type", "question_type", "difficulty", "correct_rate"])

    def test_r14_rows_start_at_1110_px_with_a_step_of_88_px_and_marks_follow_the_answer(self):
        page = planned().pages[3]
        tops = [f.box[1] for f in page.fields if f.id.endswith(".no") and ".row" in f.id]
        self.assertEqual(tops, [geometry.px(1110 + 88 * i) for i in range(3)])
        marks = [im for im in page.images if im.rel.startswith("image/mark_")]
        self.assertEqual([m.rel for m in marks], ["image/mark_o.jpg", "image/mark_x.jpg", "image/mark_o.jpg"])
        self.assertTrue(all(abs(m.rect[0] - 503.86) < 1e-9 for m in marks))

    def test_r15_charts_sit_at_the_origin_of_their_box_without_the_36_px_offset(self):
        plan = planned()
        h = geometry.PAGE_H
        self.assertAlmostEqual(h, 841.8898)
        placed = {im.rel: im.rect for p in plan.pages for im in p.images}
        self.assertAlmostEqual(placed[cb.MUNIT.rel][1], h - 697.90, places=6)
        self.assertAlmostEqual(placed[cb.TEST_LIST.rel][1], h - 577.90, places=6)
        self.assertAlmostEqual(placed[cb.SCORE_RANGE.rel][1], h - 721.89, places=6)
        grade = placed["image/course_book/1p/grade_03.jpg"]
        self.assertAlmostEqual(grade[0], 35.99, places=6)
        self.assertAlmostEqual(grade[1], h - 745.89, places=6)
        x, y0 = cb.MUNIT.point(348.5, 0)
        _, y100 = cb.MUNIT.point(348.5, 100)
        self.assertAlmostEqual(y0, (h - 697.90) + 904.5 * geometry.PX, places=9)
        self.assertAlmostEqual(y100, (h - 697.90) + 144.5 * geometry.PX, places=9)
        self.assertAlmostEqual(cb.TEST_LIST.point(0, 0)[1], (h - 577.90) + 904.5 * geometry.PX, places=9)
        # a unit marker of score 0 sits on the baseline of the chart, not 36 px lower
        dot = [im for im in plan.pages[-1].images if im.rel == cb.DOT]
        self.assertTrue(dot)
        self.assertGreater(len(plan.pages[-1].images), 3)

    def test_r16_line_break_tokens_break_and_are_not_printed(self):
        for token in ("\n", "\r\n", "\\n", "\\r\\n", "\\\\n", "\\\\r\\\\n"):
            lines = textutil.split_lines("alpha" + token + "beta")
            self.assertEqual(lines, ["alpha", "beta"], repr(token))
            self.assertEqual(textutil.single_line("alpha" + token + "beta"), "alpha beta")
        self.assertEqual(textutil.split_lines("return and warn and turn"), ["return and warn and turn"])
        plan = planned()
        words = [w for p in plan.pages for f in p.fields for w in f.text.split()]
        self.assertNotIn("rn", words)
        comment = [f.text for f in plan.pages[1].fields if f.id.startswith("p2.comment")]
        self.assertEqual(comment[:4], ["First sentence of the comment.", "Second sentence, a return visit.", "Third line.", "Fourth line."])
        data = synthetic_input()
        data["bunit_list"][0]["name"] = "Name\\r\\nwith break"
        self.assertEqual(validate_course_book(data)["bunit_list"][0]["name"], "Name with break")

    def test_r26_field_tables_are_deterministic_and_fit_their_boxes(self):
        first, second = planned(), planned()
        self.assertEqual(first.pages, second.pages)
        for page in first.pages:
            ids = [f.id for f in page.fields]
            self.assertEqual(len(ids), len(set(ids)))
            for f in page.fields:
                x0, y0, x1, y1 = f.box
                self.assertGreater(x1 - x0, 0)
                self.assertLessEqual(FAKE_CTX.measure(f.text, f.size), x1 - x0 - 2 * cb.PAD_PT + 1e-9, f.id)
                self.assertLessEqual(f.size, y1 - y0, f.id)

    def test_r26_long_values_shrink_then_are_cut_with_an_ellipsis(self):
        data = synthetic_input()
        data["bunit_list"][0]["questions"][0]["type"] = "x" * 150
        field = {f.id: f for f in planned(data).pages[3].fields}["p4.c0.row0.type"]
        self.assertTrue(field.text.endswith("..."))
        self.assertEqual(field.size, 6.0)

    def test_r26_wrap_and_number_format(self):
        self.assertEqual(textutil.format_number(70), "70")
        self.assertEqual(textutil.format_number(82.5), "82.5")
        self.assertEqual(textutil.format_number(0.04), "0.0")
        lines = textutil.wrap_text(lambda t, s: len(t) * 10, "aaa bbb ccc", 10, 70)
        self.assertEqual(lines, ["aaa bbb", "ccc"])
        self.assertEqual(geometry.hpdf_to_fitz_y(745.89), geometry.PAGE_H - 745.89)

    def test_r26_image_dimensions_are_read_from_headers(self):
        import struct

        from studyq_linux.imageinfo import image_dimensions

        png = b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", 44, 36) + b"\x08\x06\x00\x00\x00"
        self.assertEqual(image_dimensions(png), (44, 36))
        jpeg = b"\xff\xd8\xff\xe0" + struct.pack(">H", 4) + b"JF" + b"\xff\xc2" + struct.pack(">HBHH", 11, 8, 1017, 2198) + b"\x00" * 6
        self.assertEqual(image_dimensions(jpeg), (2198, 1017))
        self.assertIsNone(image_dimensions(b"GIF89a"))
        self.assertIsNone(image_dimensions(b"\xff\xd8\xff"))

    def test_r26_difficulty_page_shares_and_bars(self):
        page = planned().pages[-2]
        shares = {f.id: f.text for f in page.fields if f.id.endswith(".share")}
        self.assertEqual(shares, {"p6.top.share": "20% / 2", "p6.high.share": "30% / 3", "p6.mid.share": "40% / 4", "p6.low.share": "10% / 1"})
        self.assertEqual(len([s for s in page.shapes if s.kind == "poly"]), 4)
        self.assertEqual(len([s for s in page.shapes if s.kind == "rect"]), 4)

    def test_r26_unit_chart_marks_sit_on_the_score_rows(self):
        plan = planned()
        page = plan.pages[-1]
        bars = [s for s in page.shapes if s.fill == cb.COLOR_AVG and s.geometry[3] - s.geometry[1] > 20]
        self.assertEqual(len(bars), 5)
        first = bars[0].geometry
        self.assertAlmostEqual(first[3], cb.MUNIT.point(348.5, 0)[1], places=9)
        self.assertAlmostEqual(first[1], cb.MUNIT.point(348.5, 55)[1], places=9)


class RenderCase(TempDirCase):
    """Rendering tests: need PyMuPDF (R27)."""

    def assets(self, overflow=False):
        directory = self.path("assets")
        os.makedirs(directory, exist_ok=True)
        write_assets(directory, synthetic_contents(overflow))
        return directory

    def render(self, data=None, out=None, overflow=False):
        out = out or self.path("out.pdf")
        status, line = run_cli(
            ["render", "--type", "course_book", "--assets", self.assets(overflow), "--out", out],
            json.dumps(data or synthetic_input()).encode(),
        )
        return status, json.loads(line), out


@needs_pymupdf
class Rendering(RenderCase):
    def test_r1_success_line_and_pdf(self):
        status, doc, out = self.render()
        self.assertEqual(status, 0, doc)
        self.assertEqual(list(doc), ["status", "reportType", "pages", "sha256", "outputPath"])
        self.assertEqual((doc["status"], doc["reportType"], doc["pages"]), ("ok", "course_book", 8))
        with open(out, "rb") as handle:
            self.assertEqual(hashlib.sha256(handle.read()).hexdigest(), doc["sha256"])
        self.assertEqual(doc["outputPath"], os.path.abspath(out))
        self.assertEqual(pymupdf.open(out).page_count, 8)

    def test_r1_exactly_one_line_on_stdout_through_python_dash_m(self):
        assets = self.assets()
        env = dict(os.environ, PYTHONPATH=os.path.join(MODULE, "src"))
        env.pop("PYTHONWARNINGS", None)
        out = self.path("m.pdf")
        proc = subprocess.run(
            [sys.executable, "-B", "-m", "studyq_linux", "render", "--type", "course_book", "--assets", assets, "--out", out],
            input=json.dumps(synthetic_input()).encode(),
            env=env,
            capture_output=True,
        )
        self.assertEqual(proc.returncode, 0, proc.stdout)
        self.assertEqual(proc.stdout.count(b"\n"), 1)
        self.assertEqual(proc.stderr, b"")
        self.assertEqual(json.loads(proc.stdout)["pages"], 8)

    def test_r2_failed_run_leaves_no_file_and_keeps_an_existing_one(self):
        out = self.path("kept.pdf")
        with open(out, "wb") as handle:
            handle.write(b"previous")
        status, doc, _ = self.render(out=out)
        self.assertEqual((status, doc["code"]), (1, "output_exists"))
        with open(out, "rb") as handle:
            self.assertEqual(handle.read(), b"previous")
        data = synthetic_input()
        data["grade"] = 9  # its grade picture is not in the synthetic bundle
        status, doc, out2 = self.render(data=data, out=self.path("new.pdf"))
        self.assertEqual((status, doc["code"]), (1, "asset_missing"))
        self.assertFalse(os.path.exists(out2))
        self.assertEqual([n for n in os.listdir(self.tmp) if n.endswith(".tmp")], [])

    def test_r6_written_beside_and_renamed_never_partial(self):
        out = self.path("sub", "r.pdf")
        os.makedirs(os.path.dirname(out))
        status, doc, _ = self.render(out=out)
        self.assertEqual(status, 0)
        self.assertEqual(os.listdir(os.path.dirname(out)), ["r.pdf"])
        status, doc, _ = self.render(out=out)
        self.assertEqual(doc["code"], "output_exists")
        self.assertEqual(os.listdir(os.path.dirname(out)), ["r.pdf"])

    def test_r6_temp_file_removed_when_the_rename_loses_a_race(self):
        out = self.path("race.pdf")
        original = os.link

        def late_file(src, dst, *a, **k):
            with open(dst, "wb") as handle:
                handle.write(b"winner")
            return original(src, dst, *a, **k)

        os.link = late_file
        try:
            status, doc, _ = self.render(out=out)
        finally:
            os.link = original
        self.assertEqual(doc["code"], "output_exists")
        self.assertEqual(sorted(os.listdir(self.tmp)), ["assets", "race.pdf"])
        with open(out, "rb") as handle:
            self.assertEqual(handle.read(), b"winner")

    def test_r8_same_input_same_bytes_and_no_time_or_random_id(self):
        _, first, out1 = self.render(out=self.path("a.pdf"))
        _, second, out2 = self.render(out=self.path("b.pdf"))
        self.assertEqual(first["sha256"], second["sha256"])
        with open(out1, "rb") as handle:
            raw = handle.read()
        self.assertNotIn(b"CreationDate", raw)
        self.assertNotIn(b"ModDate", raw)
        self.assertNotIn(b"/ID", raw)
        meta = pymupdf.open(out1).metadata
        self.assertEqual(meta["creationDate"], "")
        self.assertEqual(meta["modDate"], "")

    def test_r12_only_nanum_gothic_is_embedded_on_every_page(self):
        _, doc, out = self.render()
        pdf = pymupdf.open(out)
        for page in pdf:
            fonts = page.get_fonts()
            self.assertTrue(fonts)
            self.assertEqual({f[3] for f in fonts}, {"NanumGothic Regular"})
            self.assertTrue(all(f[1] == "ttf" and f[2] == "Type0" for f in fonts))
        self.assertGreater(len(pdf.extract_font(pdf[0].get_fonts()[0][0])[-1]), 100000)

    def test_r14_rendered_page_4_text_sits_in_its_own_columns(self):
        _, doc, out = self.render()
        page = pymupdf.open(out)[3]
        words = {w[4]: w for w in page.get_text("words")}
        q = synthetic_input()["bunit_list"][0]["questions"][1]
        for name, token in (("question_type", q["question_type"]), ("difficulty", q["difficulty"])):
            x0, x1 = cb.CHAPTER_COLUMNS_PX[name]
            word = words[token]
            self.assertGreaterEqual(word[0], geometry.px(x0) - 2)
            self.assertLessEqual(word[2], geometry.px(x1) + 2)
            self.assertAlmostEqual(word[1] + (word[3] - word[1]) / 2, geometry.px(1110 + 88 * 1 + 44), delta=3)

    def test_r15_rendered_chart_is_placed_at_its_box_origin(self):
        _, doc, out = self.render()
        page = pymupdf.open(out)[7]
        info = [i for i in page.get_image_info() if i["width"] == 2198 and i["height"] == 1017]
        self.assertEqual(len(info), 1)
        self.assertAlmostEqual(info[0]["bbox"][1], geometry.PAGE_H - 697.90, delta=0.01)
        self.assertAlmostEqual(info[0]["bbox"][0], 35.99, delta=0.01)

    def test_r16_no_raw_rn_token_in_any_page(self):
        _, doc, out = self.render()
        pdf = pymupdf.open(out)
        for page in pdf:
            self.assertNotIn("rn", [w[4] for w in page.get_text("words")])
        text = pdf[1].get_text()
        self.assertIn("First sentence of the comment.", text)
        self.assertIn("return visit", text)

    def test_r17_parity_reports_zero_mismatches_and_zero_raw_rn(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context

        directory = self.assets()
        bundle = manifest.verify_assets_dir(directory)
        data = validate_course_book(synthetic_input())
        plan = cb.plan_course_book(data, make_context(pymupdf, bundle))
        _, doc, out = self.render()
        with open(out, "rb") as handle:
            result = parity.check_parity(pymupdf, handle.read(), plan)
        self.assertEqual((result["mismatches"], result["rawRn"], result["failed"]), (0, 0, []))
        self.assertGreater(result["fields"], 50)
        self.assertEqual(result["pages"], 8)

    def test_r17_parity_counts_a_displaced_field_and_an_unexplained_text(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context

        directory = self.assets()
        bundle = manifest.verify_assets_dir(directory)
        plan = cb.plan_course_book(validate_course_book(synthetic_input()), make_context(pymupdf, bundle))
        _, doc, out = self.render()
        pdf = pymupdf.open(out)
        pdf[3].insert_text((100, 700), "stray text rn", fontsize=10)
        result = parity.check_parity(pymupdf, pdf.tobytes(), plan)
        self.assertEqual(result["rawRn"], 1)
        self.assertGreaterEqual(result["mismatches"], 3)
        swapped = cb.plan_course_book(validate_course_book(synthetic_input()), make_context(pymupdf, bundle))
        page = swapped.pages[3]
        i = next(k for k, f in enumerate(page.fields) if f.id == "p4.c0.row0.difficulty")
        j = next(k for k, f in enumerate(page.fields) if f.id == "p4.c0.row0.question_type")
        a, b = page.fields[i], page.fields[j]
        page.fields[i] = cb.Field(a.id, a.text, b.box, b.align, a.size)
        page.fields[j] = cb.Field(b.id, b.text, a.box, a.align, b.size)
        with open(out, "rb") as handle:
            clean = parity.check_parity(pymupdf, handle.read(), swapped)
        named = {f for f in clean["failed"] if not f.endswith(".unexplained")}
        self.assertEqual(named, {"p4.c0.row0.difficulty", "p4.c0.row0.question_type"})
        self.assertEqual(len(clean["failed"]) - len(named), 2)  # their two words are explained by no field

    def test_r17_parity_module_prints_counts_only(self):
        directory = self.assets()
        _, doc, out = self.render()
        env = dict(os.environ, PYTHONPATH=os.path.join(MODULE, "src"))
        proc = subprocess.run(
            [sys.executable, "-B", "-m", "studyq_linux.parity", "--pdf", out, "--assets", directory],
            input=json.dumps(synthetic_input()).encode(),
            env=env,
            capture_output=True,
        )
        self.assertEqual(proc.returncode, 0, proc.stdout)
        result = json.loads(proc.stdout)
        self.assertEqual((result["status"], result["mismatches"], result["rawRn"]), ("ok", 0, 0))
        self.assertNotIn(b"Sample Learner", proc.stdout)

    def test_r29_rendered_pages_use_the_verified_template_sequence(self):
        _, doc, out = self.render(data=synthetic_input(chapters=2, questions=26), overflow=True)
        self.assertEqual(doc["pages"], 9)

    def test_r27_gate_is_open_in_this_run(self):
        self.assertTrue(HAVE_PYMUPDF)

    def test_seam_renders_from_an_already_verified_bundle_without_a_directory(self):
        from studyq_linux.render import render_course_book

        bundle = manifest.AssetBundle.from_mapping(synthetic_contents())
        pdf = render_course_book(synthetic_input(), bundle)
        self.assertEqual(pymupdf.open(stream=pdf, filetype="pdf").page_count, 8)
        self.assertEqual(pdf, render_course_book(synthetic_input(), bundle))
        with self.assertRaises(StudyqError):
            render_course_book(synthetic_input(), manifest.AssetBundle.from_mapping({manifest.FONT_REL: font_bytes()}))


@unittest.skipUnless(ASSETS_DIR and RENDER_ENABLED, "no assets directory given (STUDYQ_ASSETS_DIR)")
class OfficialTemplates(TempDirCase):
    def test_r17_parity_against_the_official_templates(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context, render_course_book

        bundle = manifest.verify_assets_dir(ASSETS_DIR)
        data = validate_course_book(synthetic_input())
        pdf = render_course_book(data, bundle)
        plan = cb.plan_course_book(data, make_context(pymupdf, bundle))
        result = parity.check_parity(pymupdf, pdf, plan)
        self.assertEqual((result["mismatches"], result["rawRn"], result["pages"]), (0, 0, 8))


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
            "from studyq_linux import course_book, cli, manifest, schema, textutil, geometry; print('ok')"
        ) % os.path.join(MODULE, "src")
        proc = subprocess.run([sys.executable, "-B", "-I", "-c", code], capture_output=True, text=True)
        self.assertEqual((proc.returncode, proc.stdout.strip()), (0, "ok"), proc.stderr)

    def test_r27_rendering_without_pymupdf_refuses_with_render_failed(self):
        code = (
            "import sys, io, json; sys.path.insert(0, %r); sys.modules['pymupdf'] = None; "
            "from studyq_linux import render; "
            "\ntry:\n render.load_pymupdf()\nexcept Exception as e:\n print(e.code, e.detail)\n"
        ) % os.path.join(MODULE, "src")
        proc = subprocess.run([sys.executable, "-B", "-I", "-c", code], capture_output=True, text=True)
        self.assertEqual(proc.stdout.strip(), "render_failed pymupdf_unavailable", proc.stderr)

    def test_r27_gate_condition_is_documented_in_the_module(self):
        self.assertEqual(RENDER_ENABLED, os.environ.get("CHECK_STUDYQ_PYMUPDF") == "1" or HAVE_PYMUPDF)


if __name__ == "__main__":
    unittest.main()
