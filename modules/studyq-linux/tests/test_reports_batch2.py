"""Tests of specs/studyq-linux/reports-batch2.md (`ganga`, `na_score`, `smart_book`). Synthetic fixtures, invented values only.

The pure-Python tests (input schemas, template sequences, field tables, the command's words) run
unconditionally (core.md R26); the rendering and parity tests run under the gate of core.md R27,
and the parity check against the official templates only when `STUDYQ_ASSETS_DIR` names an assets
directory.
"""

import io
import json
import os
import subprocess
import sys
import unittest

import test_core as core
import test_reports_batch1 as batch1
from studyq_linux import course_book, manifest
from studyq_linux.course_book import PlanContext
from studyq_linux.errors import StudyqError
from studyq_linux.geometry import PAGE_H, PAGE_W
from studyq_linux.renderers import common, formative, ganga, na_score, smart_book
from studyq_linux.schema import validate_course_book, validate_ganga, validate_na_score, validate_smart_book

TYPES = ("ganga", "na_score", "smart_book")
TEMPLATE = (2481, 3508)
IMAGE_SIZES = {"image/mark_o.jpg": (75, 56), "image/mark_x.jpg": (75, 56)}
for _n in range(1, 7):
    IMAGE_SIZES[ganga.template(_n)] = TEMPLATE
for _n in range(1, 4):
    IMAGE_SIZES[na_score.template(_n)] = TEMPLATE
for _n in range(1, 8):
    IMAGE_SIZES["image/smart_book/smartbook_%d.jpg" % _n] = TEMPLATE
for _t in range(1, 4):
    IMAGE_SIZES[ganga.pyramid_rel(_t)] = (1650, 1100)
for _k in range(1, 6):
    IMAGE_SIZES[ganga.signal_rel(_k)] = (463, 94)
for _b in range(1, 5):
    IMAGE_SIZES[na_score.band_rel(_b)] = (900, 760)
for _g in range(1, 10):
    IMAGE_SIZES["image/smart_book/1p/lv%02d.jpg" % _g] = (1040, 709)
IMAGE_SIZES.update(
    {
        ganga.GRAPH: (2009, 1347),
        ganga.AVERAGE_POINT: (37, 37),
        ganga.ONESELF_POINT: (37, 37),
        "image/smart_book/5p/score_range_chart.jpg": (2128, 261),
        "image/smart_book/5p/test_list_chart.jpg": (2198, 1017),
        "image/smart_book/4p/munit_chart.jpg": (2198, 1017),
        "image/smart_book/4p/my_score_point.png": (36, 36),
        "image/smart_book/5p/my_score_point.png": (44, 44),
        "image/smart_book/5p/avg_score_point.png": (36, 36),
    }
)
FAKE_CTX = PlanContext(lambda text, size: len(text) * size * 0.6, lambda rel: IMAGE_SIZES[rel])
COMMENT = batch1.COMMENT


def synthetic_ganga(units=4, count=20, comment=COMMENT):
    return dict(batch1.synthetic_formative(units, count, comment), title="Invented Gang-A test", pyramid_tier=2, rank_signal=4)


def synthetic_na_score(units=4, count=20, comment=COMMENT):
    data = batch1.synthetic_formative(units, count, comment)
    del data["difficulty"], data["rank_signal"]
    return dict(data, title="Invented NA score test", rank_band=3)


def synthetic_smart_book(chapters=3, count=20, comment=COMMENT):
    data = core.synthetic_input(chapters=chapters, questions=count)
    data["comment"] = comment
    return dict(data, book_code="SB-2040.inv_01")


SYNTHETIC = {"ganga": synthetic_ganga, "na_score": synthetic_na_score, "smart_book": synthetic_smart_book}
VALIDATE = {"ganga": validate_ganga, "na_score": validate_na_score, "smart_book": validate_smart_book}
PLAN = {"ganga": ganga.plan_ganga, "na_score": na_score.plan_na_score, "smart_book": smart_book.plan_smart_book}
REQUIRED = {"ganga": ganga.required_ganga_assets, "na_score": na_score.required_na_score_assets, "smart_book": smart_book.required_smart_book_assets}
QUESTION_PAGE = {"ganga": 5, "na_score": 2, "smart_book": 3}  # index of the first question page
COMMENT_PAGE = {"ganga": 1, "na_score": 1, "smart_book": 1}


def plan_of(report_type, data):
    return PLAN[report_type](VALIDATE[report_type](data), FAKE_CTX)


def all_fields(plan):
    return [f for p in plan.pages for f in p.fields]


def question_cell(plan, report_type, member):
    suffix = ".row0." + member if report_type == "smart_book" else ".q0." + member
    return next(f for f in all_fields(plan) if f.id.endswith(suffix))


class Schema(unittest.TestCase):
    def refuse(self, report_type, data):
        with self.assertRaises(StudyqError) as caught:
            VALIDATE[report_type](data)
        self.assertEqual(caught.exception.code, "input_invalid")
        return caught.exception.detail

    def test_r4_ganga_members_are_strict(self):
        out = validate_ganga(synthetic_ganga())
        self.assertEqual(
            set(out), {"title", "student", "test_date", "pyramid_tier", "my_score", "avg_score", "rank_signal", "comment", "units", "difficulty", "questions"}
        )
        self.assertEqual(out["comment"], "First invented sentence.\nSecond invented line.\nThird line.\nFourth line.")
        validate_ganga(synthetic_ganga(units=1, count=1, comment=""))
        validate_ganga(synthetic_ganga(units=12, count=100, comment="x" * 3000))
        good = synthetic_ganga()
        for name in good:
            missing = dict(good)
            del missing[name]
            self.assertEqual(self.refuse("ganga", missing), "missing:" + name)
        self.assertEqual(self.refuse("ganga", dict(good, surplus=1)), "unknown_member:input")
        for name, bad, detail in (
            ("pyramid_tier", 0, "range:pyramid_tier"),
            ("pyramid_tier", 4, "range:pyramid_tier"),
            ("pyramid_tier", 2.0, "type:pyramid_tier"),
            ("pyramid_tier", True, "type:pyramid_tier"),
            ("rank_signal", 0, "range:rank_signal"),
            ("rank_signal", 6, "range:rank_signal"),
            ("rank_signal", "3", "type:rank_signal"),
            ("my_score", 100.5, "range:my_score"),
            ("avg_score", -1, "range:avg_score"),
            ("avg_score", "61", "type:avg_score"),
            ("title", "", "range:title"),
            ("title", "t" * 121, "range:title"),
            ("student", "s" * 65, "range:student"),
            ("test_date", "d" * 33, "range:test_date"),
            ("comment", "c" * 3001, "range:comment"),
            ("comment", None, "type:comment"),
            ("units", [], "range:units"),
            ("questions", [], "range:questions"),
            ("difficulty", [], "type:difficulty"),
        ):
            self.assertEqual(self.refuse("ganga", dict(good, **{name: bad})), detail, name)
        self.assertEqual(self.refuse("ganga", synthetic_ganga(units=13)), "range:units")
        self.assertEqual(self.refuse("ganga", synthetic_ganga(count=101)), "range:questions")
        for member, bad, detail in (("name", "n" * 81, "range:units[1].name"), ("my_score", 101, "range:units[1].my_score"), ("average", "5", "type:units[1].average")):
            data = synthetic_ganga()
            data["units"][1][member] = bad
            self.assertEqual(self.refuse("ganga", data), detail)
        for member, bad, detail in (
            ("no", 0, "range:questions[2].no"),
            ("no", 501, "range:questions[2].no"),
            ("correct", 1, "type:questions[2].correct"),
            ("correct_rate", float("nan"), "range:questions[2].correct_rate"),
            ("unit", "", "range:questions[2].unit"),
            ("difficulty", "d" * 33, "range:questions[2].difficulty"),
        ):
            data = synthetic_ganga()
            data["questions"][2][member] = bad
            self.assertEqual(self.refuse("ganga", data), detail)
        data = synthetic_ganga()
        data["difficulty"]["mid"]["questions"] = 501
        self.assertEqual(self.refuse("ganga", data), "range:difficulty.mid.questions")
        self.assertEqual(self.refuse("ganga", batch1.synthetic_formative()), "missing:pyramid_tier")
        with self.assertRaises(StudyqError) as caught:
            batch1.validate_formative(synthetic_ganga())
        self.assertEqual(caught.exception.detail, "unknown_member:input")

    def test_r5_na_score_members_are_strict(self):
        out = validate_na_score(synthetic_na_score())
        self.assertEqual(set(out), {"title", "student", "test_date", "my_score", "avg_score", "rank_band", "comment", "units", "questions"})
        validate_na_score(synthetic_na_score(units=12, count=100, comment=""))
        good = synthetic_na_score()
        for name in good:
            missing = dict(good)
            del missing[name]
            self.assertEqual(self.refuse("na_score", missing), "missing:" + name)
        for name, bad, detail in (
            ("rank_band", 0, "range:rank_band"),
            ("rank_band", 5, "range:rank_band"),
            ("rank_band", "2", "type:rank_band"),
            ("rank_band", 2.5, "type:rank_band"),
            ("my_score", 101, "range:my_score"),
            ("comment", 7, "type:comment"),
        ):
            self.assertEqual(self.refuse("na_score", dict(good, **{name: bad})), detail, name)
        self.assertEqual(self.refuse("na_score", dict(good, difficulty=batch1.LEVEL_INPUT)), "unknown_member:input")
        self.assertEqual(self.refuse("na_score", dict(good, rank_signal=1)), "unknown_member:input")
        self.assertEqual(self.refuse("na_score", synthetic_na_score(units=13)), "range:units")
        data = synthetic_na_score()
        data["questions"][0]["area"] = data["questions"][0].pop("unit")
        self.assertEqual(self.refuse("na_score", data), "missing:questions[0].unit")
        self.assertEqual(self.refuse("na_score", synthetic_ganga()), "missing:rank_band")
        self.assertEqual(self.refuse("ganga", synthetic_na_score()), "missing:pyramid_tier")

    def test_r6_smart_book_adds_book_code_to_the_course_book_members(self):
        out = validate_smart_book(synthetic_smart_book())
        self.assertEqual(set(out), set(validate_course_book(core.synthetic_input())) | {"book_code"})
        self.assertEqual(out["book_code"], "SB-2040.inv_01")
        without = dict(out)
        del without["book_code"]
        self.assertEqual(without, validate_course_book(dict(core.synthetic_input(questions=20), comment=COMMENT)))
        for code in ("a", "A" * 64, "0.9_-x"):
            validate_smart_book(dict(synthetic_smart_book(), book_code=code))
        for bad, detail in (("", "range:book_code"), ("a" * 65, "range:book_code"), ("SB 1", "range:book_code"), ("SB/1", "range:book_code"),
                            ("책1", "range:book_code"), ("SB-1\n", "range:book_code"), (12, "type:book_code"), (None, "type:book_code")):
            self.assertEqual(self.refuse("smart_book", dict(synthetic_smart_book(), book_code=bad)), detail, bad)
        good = synthetic_smart_book()
        for name in good:
            missing = dict(good)
            del missing[name]
            self.assertEqual(self.refuse("smart_book", missing), "missing:" + name)
        self.assertEqual(self.refuse("smart_book", dict(good, surplus=1)), "unknown_member:input")
        self.assertEqual(self.refuse("smart_book", dict(good, grade=10)), "range:grade")
        self.assertEqual(self.refuse("smart_book", core.synthetic_input()), "missing:book_code")
        self.assertEqual(self.refuse("smart_book", []), "type:")
        with self.assertRaises(StudyqError) as caught:
            validate_course_book(synthetic_smart_book())
        self.assertEqual(caught.exception.detail, "unknown_member:input")

    def test_r7_details_hold_no_input_value(self):
        value = "Zq" + "Xv" + "Learner-Name"
        for report_type in TYPES:
            data = SYNTHETIC[report_type]()
            data["student"] = value * 9
            self.assertEqual(self.refuse(report_type, data), "range:student")
            data = SYNTHETIC[report_type]()
            data[value] = value
            self.assertNotIn(value, self.refuse(report_type, data))
            data = SYNTHETIC[report_type]()
            rows = data["bunit_list"][0]["questions"] if report_type == "smart_book" else data["questions"]
            rows[0][value] = 1
            self.assertNotIn(value, self.refuse(report_type, data))
        detail = self.refuse("smart_book", dict(synthetic_smart_book(), book_code=value + " x"))
        self.assertEqual(detail, "range:book_code")


class Layout(unittest.TestCase):
    def test_r1_template_sequences(self):
        g, n = "image/ganga/ganga_%d.jpg", "image/na_score/score_%d.jpg"
        for count, extra in ((1, 0), (25, 0), (26, 1), (50, 1), (51, 2), (100, 3)):
            seq = [p.template for p in plan_of("ganga", synthetic_ganga(count=count)).pages]
            self.assertEqual(seq, [g % k for k in range(1, 7)] + [g % 6] * extra, count)
            self.assertEqual(seq, ganga.template_sequence(count))
            seq = [p.template for p in plan_of("na_score", synthetic_na_score(count=count)).pages]
            self.assertEqual(seq, [n % 1, n % 2, n % 3] + [n % 3] * extra, count)
            self.assertEqual(seq, na_score.template_sequence(count))
        s = "image/smart_book/smartbook_%d.jpg"
        self.assertEqual([p.template for p in plan_of("smart_book", synthetic_smart_book()).pages], [s % k for k in (1, 2, 3, 4, 4, 4, 6, 7)])
        data = synthetic_smart_book(chapters=2)
        data["bunit_list"][0]["questions"] = data["bunit_list"][0]["questions"] * 3  # 60 rows: 4, 5, 5
        seq = [p.template for p in plan_of("smart_book", data).pages]
        self.assertEqual(seq, [s % k for k in (1, 2, 3, 4, 5, 5, 4, 6, 7)])
        self.assertEqual(seq, smart_book.template_sequence([60, 20]))
        self.assertEqual(len(plan_of("ganga", synthetic_ganga()).pages), 6)
        self.assertEqual(len(plan_of("na_score", synthetic_na_score()).pages), 3)
        self.assertEqual(len(plan_of("smart_book", synthetic_smart_book()).pages), 8)

    def test_r2_required_assets_are_the_official_overlays(self):
        rels = ganga.required_ganga_assets(validate_ganga(synthetic_ganga()))
        self.assertEqual(rels, sorted(set(rels)))
        for rel in ("image/ganga/1p/01 pyramid2.jpg", "image/ganga/3p/03 graph.jpg", "image/ganga/3p/03 average-point.png", "image/ganga/3p/03 oneself-point.png",
                    "image/ganga/5p/05 signal_04.jpg", "image/ganga/ganga_6.jpg", "image/mark_o.jpg", "image/mark_x.jpg"):
            self.assertIn(rel, rels)
        self.assertFalse([r for r in rels if r.startswith("image/formative/")])
        rels = na_score.required_na_score_assets(validate_na_score(synthetic_na_score()))
        self.assertEqual(rels, ["image/mark_o.jpg", "image/mark_x.jpg", "image/na_score/03.jpg", "image/na_score/score_1.jpg", "image/na_score/score_2.jpg", "image/na_score/score_3.jpg"])
        rels = smart_book.required_smart_book_assets(validate_smart_book(synthetic_smart_book()))
        self.assertEqual(
            rels,
            sorted(
                ["image/smart_book/smartbook_%d.jpg" % k for k in (1, 2, 3, 4, 6, 7)]
                + ["image/smart_book/1p/lv03.jpg", "image/smart_book/4p/munit_chart.jpg", "image/smart_book/4p/my_score_point.png", "image/smart_book/5p/score_range_chart.jpg",
                   "image/smart_book/5p/test_list_chart.jpg", "image/smart_book/5p/my_score_point.png", "image/smart_book/5p/avg_score_point.png", "image/mark_o.jpg", "image/mark_x.jpg"]
            ),
        )
        self.assertEqual(smart_book.asset_rel("image/course_book/1p/grade_09.jpg"), "image/smart_book/1p/lv09.jpg")
        for tier in (1, 3):
            self.assertIn("image/ganga/1p/01 pyramid%d.jpg" % tier, ganga.required_ganga_assets(validate_ganga(dict(synthetic_ganga(), pyramid_tier=tier))))
        for band in (1, 4):
            self.assertIn("image/na_score/0%d.jpg" % band, na_score.required_na_score_assets(validate_na_score(dict(synthetic_na_score(), rank_band=band))))
        # Every picture a plan draws is a required asset, and no asset of another report is read.
        for report_type in TYPES:
            data = VALIDATE[report_type](SYNTHETIC[report_type](count=60))
            plan = PLAN[report_type](data, FAKE_CTX)
            used = {p.template for p in plan.pages} | {i.rel for p in plan.pages for i in p.images}
            self.assertEqual(used - set(REQUIRED[report_type](data)), set(), report_type)
            own = "image/%s/" % report_type
            self.assertEqual({r for r in used if not r.startswith(own)}, {"image/mark_o.jpg", "image/mark_x.jpg"}, report_type)

    def test_r3_fields_fit_their_boxes_inside_a4_without_overlap(self):
        for report_type in TYPES:
            for kwargs in ({}, {"count": 100}, {"count": 1, "comment": ""}, {"units": 12} if report_type != "smart_book" else {"chapters": 1}):
                plan = plan_of(report_type, SYNTHETIC[report_type](**kwargs))
                for page in plan.pages:
                    for f in page.fields:
                        x0, y0, x1, y1 = f.box
                        self.assertTrue(0 <= x0 < x1 <= PAGE_W and 0 <= y0 < y1 <= PAGE_H, (report_type, f.id))
                        self.assertLessEqual(FAKE_CTX.measure(f.text, f.size), x1 - x0 - 2 + 1e-9, f.id)
                        self.assertGreaterEqual(f.size, 6.0)
                        self.assertTrue(f.text)
                    boxes = [(f.box, f.id) for f in page.fields]
                    for i, (a, ia) in enumerate(boxes):
                        for b, ib in boxes[i + 1 :]:
                            overlap = a[0] < b[2] - 1e-6 and b[0] < a[2] - 1e-6 and a[1] < b[3] - 1e-6 and b[1] < a[3] - 1e-6
                            self.assertFalse(overlap, (report_type, ia, ib))
                    for image in page.images:
                        r = image.rect
                        self.assertTrue(0 <= r[0] < r[2] <= PAGE_W + 1e-6 and 0 <= r[1] < r[3] <= PAGE_H + 1e-6, (report_type, image.rel))
                ids = [f.id for f in all_fields(plan)]
                self.assertEqual(len(ids), len(set(ids)))

    def test_r3_long_values_shrink_to_6pt_then_are_cut(self):
        for report_type in TYPES:
            data = SYNTHETIC[report_type]()
            if report_type == "smart_book":
                data["student"] = "S" * 64
                data["bunit_list"][0]["questions"][0]["difficulty"] = "D" * 32
                head = next(f for f in plan_of(report_type, data).pages[0].fields if f.id == "p1.student")
            else:
                data["title"] = "T" * 120
                data["questions"][0]["difficulty"] = "D" * 32
                head = next(f for f in plan_of(report_type, data).pages[0].fields if f.id == "p1.title")
            plan = plan_of(report_type, data)
            self.assertEqual(head.size, 6.0)
            self.assertTrue(head.text.endswith("..."))
            cell = question_cell(plan, report_type, "difficulty")
            self.assertEqual(cell.size, 6.0, report_type)
            self.assertTrue(cell.text.endswith("..."))

    def test_r3_comment_is_wrapped_and_cut_with_an_ellipsis(self):
        for report_type in TYPES:
            page_no = COMMENT_PAGE[report_type]
            plan = plan_of(report_type, SYNTHETIC[report_type](comment=" ".join(["word"] * 600)))
            lines = [f for f in plan.pages[page_no].fields if ".comment." in f.id]
            self.assertGreater(len(lines), 3)
            self.assertTrue(lines[-1].text.endswith("..."))
            self.assertEqual(len({f.box[0] for f in lines}), 1)
            short = [f for f in plan_of(report_type, SYNTHETIC[report_type]()).pages[page_no].fields if ".comment." in f.id]
            self.assertEqual([f.text for f in short], ["First invented sentence.", "Second invented line.", "Third line.", "Fourth line."])
            empty = plan_of(report_type, SYNTHETIC[report_type](comment=""))
            self.assertFalse([f for f in all_fields(empty) if ".comment." in f.id])

    def test_r9_line_break_tokens_break_and_no_raw_rn_is_planned(self):
        for report_type in TYPES:
            data = SYNTHETIC[report_type](comment="alpha\\r\\nbeta\\\\ngamma\r\ndelta\nepsilon")
            head = "student" if report_type == "smart_book" else "title"
            data[head] = "Invented\\r\\nvalue"
            rows = data["bunit_list"][0]["questions"] if report_type == "smart_book" else data["questions"]
            rows[0]["difficulty"] = "Hi\\ngh"
            fields = all_fields(plan_of(report_type, data))
            self.assertEqual([f.text for f in fields if ".comment." in f.id], ["alpha", "beta", "gamma", "delta", "epsilon"])
            self.assertEqual(next(f.text for f in fields if f.id == "p1." + head), "Invented value")
            self.assertEqual(question_cell(plan_of(report_type, data), report_type, "difficulty").text, "Hi gh")
            for f in fields:
                self.assertNotIn("\\", f.text)
                self.assertNotIn("\n", f.text)
                self.assertNotIn("rn", f.text.split())
                self.assertNotEqual(f.text, "")

    def test_r3_question_rows_columns_and_marks(self):
        px = core.geometry.px
        plan = plan_of("ganga", synthetic_ganga(count=27))
        first, second = plan.pages[5], plan.pages[6]
        by_id = {f.id: f for f in first.fields}
        self.assertEqual(by_id["p6.q0.no"].box[:2], (px(212), px(1794)))
        self.assertEqual(by_id["p6.q15.no"].box[:2], (px(212 + 1104), px(1794)))
        self.assertEqual(by_id["p6.q24.no"].box[1], px(1794 + 9 * 98))
        self.assertEqual(first.images[0].rel, "image/ganga/5p/05 signal_04.jpg")
        self.assertEqual(len(first.images), 26)
        self.assertNotIn(first.images[0].rel, [i.rel for i in second.images])
        self.assertEqual(sorted(f.id for f in second.fields if f.id.endswith(".no")), ["p7.q25.no", "p7.q26.no"])
        self.assertFalse([f for f in all_fields(plan) if f.text.startswith("Unit") and f.id.startswith(("p6.", "p7."))])
        plan = plan_of("na_score", synthetic_na_score(count=27))
        first, second = plan.pages[2], plan.pages[3]
        by_id = {f.id: f for f in first.fields}
        self.assertEqual(by_id["p3.q0.no"].box, (px(247), px(334), px(425), px(432)))
        self.assertEqual(by_id["p3.q24.unit"].box[1], px(334 + 24 * 98))
        self.assertEqual((by_id["p3.q1.unit"].align, by_id["p3.q1.unit"].text), ("l", "Unit 2"))
        self.assertEqual(by_id["p3.q3.correct_rate"].text, "11.5")
        self.assertEqual([i.rel for i in first.images][:3], ["image/mark_x.jpg", "image/mark_o.jpg", "image/mark_o.jpg"])
        self.assertEqual(len(first.images), 25)
        self.assertEqual(sorted(f.id for f in second.fields if f.id.endswith(".no")), ["p4.q25.no", "p4.q26.no"])
        self.assertEqual(second.fields[0].box[1], first.fields[0].box[1])
        plan = plan_of("smart_book", synthetic_smart_book(chapters=1, count=27))
        chapter, overflow = plan.pages[3], plan.pages[4]
        self.assertEqual(len([i for i in chapter.images if i.rel.startswith("image/mark_")]), 25)
        self.assertEqual(sorted(f.id for f in overflow.fields if f.id.endswith(".no")), ["p5.c0.1.row0.no", "p5.c0.1.row1.no"])

    def test_r3_unit_counts_come_from_the_questions_only(self):
        data = validate_ganga(synthetic_ganga(units=6, count=20))
        self.assertEqual(formative.unit_counts(data), [5, 5, 5, 5, 0, 0])
        plan = ganga.plan_ganga(data, FAKE_CTX)
        self.assertEqual(sorted(f.id for f in plan.pages[2].fields if f.id.startswith("p3.u")), ["p3.u%d.questions" % i for i in range(4)])
        self.assertEqual({f.id: f.text for f in plan.pages[2].fields}["p3.questions"], "20")
        self.assertEqual(len([f for f in plan.pages[3].fields if f.id.endswith(".name")]), 6)
        self.assertEqual([i.rel for i in plan.pages[3].images], [ganga.GRAPH] + [ganga.AVERAGE_POINT, ganga.ONESELF_POINT] * 4)
        self.assertEqual(plan.pages[4].fields + plan.pages[4].images, [])
        plan = plan_of("na_score", synthetic_na_score(units=6))
        table = {f.id: f.text for f in plan.pages[1].fields if f.id.startswith("p2.u")}
        self.assertEqual([table["p2.u%d.name" % i] for i in range(6)], ["Unit %d" % (i + 1) for i in range(6)])
        self.assertEqual(len(table), 18)  # one row a unit, none invented

    def test_r3_scores_and_pictures_follow_the_input(self):
        px = core.geometry.px
        plan = plan_of("ganga", synthetic_ganga())
        results = {f.id: f.text for f in plan.pages[1].fields}
        self.assertEqual((results["p2.my_score"], results["p2.avg_score"]), ("80", "66.5"))
        self.assertNotIn("p2.rank_signal", results)
        self.assertEqual([i.rel for i in plan.pages[1].images], ["image/ganga/1p/01 pyramid2.jpg"])
        self.assertEqual({f.id: f.text for f in plan.pages[0].fields}, {"p1.title": "Invented Gang-A test", "p1.test_date": "2040-03-02", "p1.student": "Sample Learner"})
        level_bars = [s for s in plan.pages[3].shapes if s.kind == "rect"]
        self.assertEqual(len(level_bars), 4)
        self.assertEqual(len([s for s in plan.pages[2].shapes if s.kind == "poly"]), 4)
        plan = plan_of("na_score", synthetic_na_score())
        results = {f.id: f.text for f in plan.pages[1].fields}
        self.assertEqual((results["p2.my_score"], results["p2.avg_score"]), ("80", "66.5"))
        self.assertEqual([i.rel for i in plan.pages[1].images], ["image/na_score/03.jpg"])
        bars = [s for s in plan.pages[1].shapes if s.kind == "rect"]
        self.assertAlmostEqual(bars[0].geometry[2] - bars[0].geometry[0], px(na_score.BAR_PX_PER_POINT * 80))
        self.assertEqual(len(plan_of("na_score", dict(synthetic_na_score(), my_score=0)).pages[1].shapes), 1)

    def test_r3_smart_book_is_the_course_book_layout_on_its_own_templates(self):
        data = validate_smart_book(synthetic_smart_book())
        mine = smart_book.plan_smart_book(data, FAKE_CTX)
        sizes = dict(core.IMAGE_SIZES)
        for rel in list(IMAGE_SIZES):
            if rel.startswith("image/smart_book/"):
                rest = rel[len("image/smart_book/") :].replace("smartbook_", "coursebook_")
                if rest.startswith("1p/lv"):
                    rest = "1p/grade_%s" % rest[len("1p/lv") :]
                sizes[course_book.TEMPLATE_DIR + rest] = IMAGE_SIZES[rel]
        theirs = course_book.plan_course_book(data, PlanContext(FAKE_CTX.measure, lambda rel: sizes[rel]))
        self.assertEqual([p.fields for p in mine.pages], [p.fields for p in theirs.pages])
        self.assertEqual([p.shapes for p in mine.pages], [p.shapes for p in theirs.pages])
        self.assertEqual([[i.rect for i in p.images] for p in mine.pages], [[i.rect for i in p.images] for p in theirs.pages])
        self.assertEqual([i.rel for i in mine.pages[1].images], ["image/smart_book/1p/lv03.jpg"])
        self.assertFalse([f for f in all_fields(mine) if "SB-2040" in f.text])  # book_code is not printed

    def test_r8_the_plan_is_deterministic(self):
        for report_type in TYPES:
            self.assertEqual(plan_of(report_type, SYNTHETIC[report_type](count=60)), plan_of(report_type, SYNTHETIC[report_type](count=60)))

    def test_batch1_layouts_are_unchanged_by_the_shared_pieces(self):
        # formative and diag lend their pages to ganga and na_score; their own plans keep their pages and ids.
        plan = batch1.plan_of("formative", batch1.synthetic_formative(count=27))
        self.assertEqual([p.template for p in plan.pages], ["image/formative/formative_%d.jpg" % k for k in (1, 2, 3, 4, 5, 6, 6)])
        self.assertEqual(plan.pages[5].images[0].rel, "image/formative/6p/signal_3.jpg")
        self.assertEqual(plan.pages[3].images[0].rel, "image/formative/4p/03 graph.jpg")
        ids = {f.id for f in batch1.all_fields(batch1.plan_of("diag", batch1.synthetic_diag()))}
        self.assertIn("p9.q0.area", ids)
        self.assertNotIn("p9.q0.unit", ids)


class Command(core.TempDirCase):
    def test_r1_types_are_known_and_refuse_the_other_types_inputs(self):
        sources = dict(SYNTHETIC, course_book=core.synthetic_input, **batch1.SYNTHETIC)
        for report_type in TYPES:
            status, line = core.run_cli(["render", "--type", report_type, "--assets", self.tmp, "--out", self.path("o.pdf")], b"{}")
            self.assertEqual((status, json.loads(line)["code"]), (1, "input_invalid"))
            status, line = core.run_cli(["render", "--type", report_type, "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(SYNTHETIC[report_type]()).encode())
            self.assertEqual(json.loads(line)["code"], "asset_manifest_invalid")
            for other, make in sources.items():
                if other == report_type:
                    continue
                status, line = core.run_cli(["render", "--type", report_type, "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(make()).encode())
                self.assertEqual(json.loads(line)["code"], "input_invalid", (report_type, other))
                if other in batch1.TYPES or other == "course_book":
                    status, line = core.run_cli(["render", "--type", other, "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(SYNTHETIC[report_type]()).encode())
                    self.assertEqual(json.loads(line)["code"], "input_invalid", (other, report_type))
        self.assertEqual(os.listdir(self.tmp), [])

    def test_r10_parity_module_takes_the_new_types(self):
        from studyq_linux import parity

        out = io.StringIO()
        saved = sys.stdout, sys.stdin
        try:
            sys.stdout = out
            for report_type in TYPES:
                sys.stdin = io.TextIOWrapper(io.BytesIO(b"{}"))  # never the terminal's standard input
                self.assertEqual(parity.main(["--pdf", self.path("absent.pdf"), "--assets", self.tmp, "--type", report_type]), 1)
            self.assertEqual(parity.main(["--pdf", "p", "--assets", "a", "--type", "gang_a"]), 2)
        finally:
            sys.stdout, sys.stdin = saved
        codes = [json.loads(line)["code"] for line in out.getvalue().splitlines()]
        self.assertEqual(codes, ["input_invalid"] * 3 + ["usage_invalid"])


_CONTENTS = {}


def report_contents():
    """Synthetic templates (small) and overlays (their real sizes) plus the repository font."""
    if not _CONTENTS:
        _CONTENTS[manifest.FONT_REL] = core.font_bytes()
        cache = {}
        for rel, (w, h) in IMAGE_SIZES.items():
            size = (248, 351) if (w, h) == TEMPLATE else (w, h)
            fmt = "png" if rel.endswith(".png") else "jpg"
            if (size, fmt) not in cache:
                cache[(size, fmt)] = core.picture(size[0], size[1], fmt)
            _CONTENTS[rel] = cache[(size, fmt)]
    return dict(_CONTENTS)


@core.needs_pymupdf
class Rendering(core.TempDirCase):
    def assets(self, contents=None):
        directory = self.path("assets")
        if not os.path.isdir(directory):
            os.makedirs(directory)
            core.write_assets(directory, contents or report_contents())
        return directory

    def render(self, report_type, data, out=None):
        out = out or self.path("%s.pdf" % report_type)
        status, line = core.run_cli(["render", "--type", report_type, "--assets", self.assets(), "--out", out], json.dumps(data).encode())
        return status, json.loads(line), out

    def parity(self, report_type, data, out):
        env = dict(os.environ, PYTHONPATH=os.path.join(core.MODULE, "src"))
        proc = subprocess.run(
            [sys.executable, "-B", "-m", "studyq_linux.parity", "--pdf", out, "--assets", self.assets(), "--type", report_type],
            input=json.dumps(data).encode(),
            env=env,
            capture_output=True,
        )
        return proc.returncode, json.loads(proc.stdout)

    def test_r1_rendered_page_counts_follow_the_template_sequence(self):
        for report_type, base in (("ganga", 6), ("na_score", 3)):
            for count, extra in ((20, 0), (26, 1)):
                status, doc, _ = self.render(report_type, SYNTHETIC[report_type](count=count), self.path("%s-%d.pdf" % (report_type, count)))
                self.assertEqual(status, 0, doc)
                self.assertEqual(list(doc), ["status", "reportType", "pages", "sha256", "outputPath"])
                self.assertEqual((doc["reportType"], doc["pages"]), (report_type, base + extra))
        for count, pages in ((20, 8), (26, 11)):
            status, doc, _ = self.render("smart_book", synthetic_smart_book(count=count), self.path("smart_book-%d.pdf" % count))
            self.assertEqual(status, 0, doc)
            self.assertEqual((doc["reportType"], doc["pages"]), ("smart_book", pages))

    def test_r10_parity_reports_zero_mismatches_and_zero_raw_rn(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context

        for report_type in TYPES:
            data = SYNTHETIC[report_type](count=60, comment="Invented comment.\\r\\nSecond line of it.\n" + "word " * 300)
            if report_type != "smart_book":
                data["units"] = batch1.synthetic_formative(units=12)["units"]
            bundle = manifest.verify_assets_dir(self.assets())
            plan = PLAN[report_type](VALIDATE[report_type](data), make_context(core.pymupdf, bundle))
            status, doc, out = self.render(report_type, data)
            self.assertEqual(status, 0, doc)
            with open(out, "rb") as handle:
                result = parity.check_parity(core.pymupdf, handle.read(), plan)
            self.assertEqual((result["mismatches"], result["rawRn"], result["failed"]), (0, 0, []), report_type)
            self.assertEqual(result["pages"], len(plan.pages))
            code, printed = self.parity(report_type, data, out)
            self.assertEqual((code, printed["status"], printed["mismatches"], printed["rawRn"]), (0, "ok", 0, 0), report_type)
            self.assertNotIn("Learner", json.dumps(printed))

    def test_r10_parity_counts_a_displaced_field_and_a_raw_rn(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context

        for report_type in TYPES:
            data = SYNTHETIC[report_type]()
            bundle = manifest.verify_assets_dir(self.assets())
            plan = PLAN[report_type](VALIDATE[report_type](data), make_context(core.pymupdf, bundle))
            _, _, out = self.render(report_type, data)
            pdf = core.pymupdf.open(out)
            pdf[0].insert_text((100, 700), "stray rn", fontsize=10)
            result = parity.check_parity(core.pymupdf, pdf.tobytes(), plan)
            self.assertEqual(result["rawRn"], 1)
            self.assertGreaterEqual(result["mismatches"], 1)

    def test_r8_same_input_same_bytes_and_no_time_or_id(self):
        for report_type in TYPES:
            _, first, out = self.render(report_type, SYNTHETIC[report_type](), self.path("%s-a.pdf" % report_type))
            _, second, _ = self.render(report_type, SYNTHETIC[report_type](), self.path("%s-b.pdf" % report_type))
            self.assertEqual(first["sha256"], second["sha256"])
            with open(out, "rb") as handle:
                raw = handle.read()
            self.assertNotIn(b"CreationDate", raw)
            self.assertNotIn(b"ModDate", raw)
            self.assertNotIn(b"/ID", raw)
            for page in core.pymupdf.open(out):
                self.assertEqual({f[3] for f in page.get_fonts()}, {"NanumGothic Regular"})

    def test_r2_missing_overlay_refuses_with_asset_missing(self):
        from studyq_linux.render import render_report

        for report_type, rel in (("ganga", "image/ganga/1p/01 pyramid2.jpg"), ("ganga", "image/ganga/5p/05 signal_04.jpg"), ("na_score", "image/na_score/03.jpg"),
                                 ("smart_book", "image/smart_book/1p/lv03.jpg"), ("smart_book", "image/smart_book/4p/munit_chart.jpg")):
            contents = report_contents()
            del contents[rel]
            with self.assertRaises(StudyqError) as caught:
                render_report(report_type, SYNTHETIC[report_type](), manifest.AssetBundle.from_mapping(contents))
            self.assertEqual(caught.exception.code, "asset_missing", rel)


@unittest.skipUnless(core.ASSETS_DIR and core.RENDER_ENABLED, "no assets directory given (STUDYQ_ASSETS_DIR)")
class OfficialTemplates(core.TempDirCase):
    def test_r10_parity_against_the_official_templates(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context, render_report

        bundle = manifest.verify_assets_dir(core.ASSETS_DIR)
        for report_type in TYPES:
            for count in (20, 100):
                data = VALIDATE[report_type](SYNTHETIC[report_type](count=count, comment="Invented comment.\n" + "word " * 400))
                pdf = render_report(report_type, data, bundle)
                plan = PLAN[report_type](data, make_context(core.pymupdf, bundle))
                result = parity.check_parity(core.pymupdf, pdf, plan)
                self.assertEqual((result["mismatches"], result["rawRn"], result["pages"]), (0, 0, len(plan.pages)), (report_type, count, result["failed"]))


if __name__ == "__main__":
    unittest.main()
