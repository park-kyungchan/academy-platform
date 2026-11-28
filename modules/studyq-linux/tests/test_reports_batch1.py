"""Tests of specs/studyq-linux/reports-batch1.md (`diag`, `new_diag`, `formative`). Synthetic fixtures, invented values only.

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
from studyq_linux import manifest
from studyq_linux.course_book import PlanContext
from studyq_linux.errors import StudyqError
from studyq_linux.geometry import PAGE_H, PAGE_W
from studyq_linux.renderers import common, diag, formative, new_diag
from studyq_linux.schema import MBTI_CODES, validate_diag, validate_formative, validate_new_diag

TYPES = ("diag", "new_diag", "formative")
TEMPLATE = (2481, 3508)
IMAGE_SIZES = {"image/mark_o.jpg": (75, 56), "image/mark_x.jpg": (75, 56)}
for _n in range(1, 10):
    IMAGE_SIZES["image/diag/diag_%d.jpg" % _n] = TEMPLATE
for _n in range(1, 11):
    IMAGE_SIZES["image/new_diag/diag_%d.jpg" % _n] = TEMPLATE
for _n in range(1, 7):
    IMAGE_SIZES["image/formative/formative_%d.jpg" % _n] = TEMPLATE
for _g in range(1, 10):
    IMAGE_SIZES[diag.grade_rel(_g)] = (1016, 1017)
for _s in ("green", "yellow", "red"):
    IMAGE_SIZES[diag.light_rel(_s)] = (242, 84)
for _r in ("nice", "normal", "base", "fail"):
    IMAGE_SIZES[diag.rating_rel(_r)] = (558, 828)
for _m in MBTI_CODES:
    IMAGE_SIZES[new_diag.mbti_rel(_m)] = (20, 35) if _m == "other" else (2080, 415)
for _k in range(1, 6):
    IMAGE_SIZES[formative.signal_rel(_k)] = (463, 94)
IMAGE_SIZES.update(
    {
        "image/diag/4p/area_graph.jpg": (1985, 1442),
        "image/diag/4p/03_graph.jpg": (1985, 1442),
        diag.POINT: (37, 37),
        diag.DIFFICULTY_CHART: (1914, 639),
        formative.SCORE_CHART: (2052, 684),
        formative.GRAPH: (2009, 1347),
        formative.AVERAGE_POINT: (37, 37),
        formative.ONESELF_POINT: (37, 37),
    }
)
FAKE_CTX = PlanContext(lambda text, size: len(text) * size * 0.6, lambda rel: IMAGE_SIZES[rel])
LEVEL_INPUT = {"top": {"questions": 2, "score": 40}, "high": {"questions": 5, "score": 55.5}, "mid": {"questions": 8, "score": 70}, "low": {"questions": 5, "score": 100}}
COMMENT = "First invented sentence.\\r\\nSecond invented line.\nThird line.\r\nFourth line."


def questions(count, group, groups=4):
    return [
        {
            "no": i + 1,
            group: "%s %d" % ("Area" if group == "area" else "Unit", i % groups + 1),
            "difficulty": ["Top", "High", "Mid", "Low"][i % 4],
            "correct_rate": (37 * i) % 100 + 0.5 * (i % 2),
            "correct": i % 3 != 0,
        }
        for i in range(count)
    ]


def synthetic_diag(areas=4, count=20, comment=COMMENT):
    return {
        "title": "Invented diagnostic test",
        "student": "Sample Learner",
        "test_date": "2040-03-02",
        "grade": 4,
        "total_score": 72.5,
        "avg_score": 61,
        "comment": comment,
        "areas": [
            {"name": "Area %d" % (i + 1), "score": 40 + 7 * i, "average": 60, "signal": ("green", "yellow", "red")[i % 3], "rating": ("nice", "normal", "base", "fail")[i % 4]}
            for i in range(areas)
        ],
        "difficulty": json.loads(json.dumps(LEVEL_INPUT)),
        "questions": questions(count, "area"),
    }


def synthetic_new_diag(areas=4, count=20, mbti="INTP", comment=COMMENT):
    return dict(synthetic_diag(areas, count, comment), apply_code="Invented-Code-7", mbti=mbti)


def synthetic_formative(units=4, count=20, comment=COMMENT):
    return {
        "title": "Invented formative test",
        "student": "Sample Learner",
        "test_date": "2040-03-02",
        "my_score": 80,
        "avg_score": 66.5,
        "rank_signal": 3,
        "comment": comment,
        "units": [{"name": "Unit %d" % (i + 1), "my_score": 30 + 6 * i, "average": 55} for i in range(units)],
        "difficulty": json.loads(json.dumps(LEVEL_INPUT)),
        "questions": questions(count, "unit"),
    }


SYNTHETIC = {"diag": synthetic_diag, "new_diag": synthetic_new_diag, "formative": synthetic_formative}
VALIDATE = {"diag": validate_diag, "new_diag": validate_new_diag, "formative": validate_formative}
PLAN = {"diag": diag.plan_diag, "new_diag": new_diag.plan_new_diag, "formative": formative.plan_formative}
REQUIRED = {"diag": diag.required_diag_assets, "new_diag": new_diag.required_new_diag_assets, "formative": formative.required_formative_assets}


def plan_of(report_type, data):
    return PLAN[report_type](VALIDATE[report_type](data), FAKE_CTX)


def all_fields(plan):
    return [f for p in plan.pages for f in p.fields]


class Schema(unittest.TestCase):
    def refuse(self, report_type, data):
        with self.assertRaises(StudyqError) as caught:
            VALIDATE[report_type](data)
        self.assertEqual(caught.exception.code, "input_invalid")
        return caught.exception.detail

    def test_r4_diag_good_input_is_accepted_and_normalised(self):
        out = validate_diag(synthetic_diag())
        self.assertEqual(set(out), {"title", "student", "test_date", "grade", "total_score", "avg_score", "comment", "areas", "difficulty", "questions"})
        self.assertEqual(out["comment"], "First invented sentence.\nSecond invented line.\nThird line.\nFourth line.")
        validate_diag(synthetic_diag(areas=1, count=1, comment=""))
        validate_diag(synthetic_diag(areas=8, count=100, comment="x" * 3000))
        data = synthetic_diag()
        data["difficulty"]["top"]["questions"] = 500
        data["questions"][0]["no"] = 500
        validate_diag(data)

    def test_r4_diag_members_are_strict(self):
        good = synthetic_diag()
        for name in good:
            missing = dict(good)
            del missing[name]
            self.assertEqual(self.refuse("diag", missing), "missing:" + name)
        self.assertEqual(self.refuse("diag", dict(good, surplus=1)), "unknown_member:input")
        self.assertEqual(self.refuse("diag", dict(good, apply_code="x")), "unknown_member:input")
        for name, bad, detail in (
            ("grade", 0, "range:grade"),
            ("grade", 10, "range:grade"),
            ("grade", 4.0, "type:grade"),
            ("grade", True, "type:grade"),
            ("total_score", 100.5, "range:total_score"),
            ("avg_score", -1, "range:avg_score"),
            ("avg_score", "61", "type:avg_score"),
            ("title", "", "range:title"),
            ("title", "t" * 121, "range:title"),
            ("student", "s" * 65, "range:student"),
            ("test_date", "d" * 33, "range:test_date"),
            ("comment", "c" * 3001, "range:comment"),
            ("comment", None, "type:comment"),
            ("areas", [], "range:areas"),
            ("questions", [], "range:questions"),
            ("difficulty", [], "type:difficulty"),
        ):
            self.assertEqual(self.refuse("diag", dict(good, **{name: bad})), detail, name)
        self.assertEqual(self.refuse("diag", synthetic_diag(areas=9)), "range:areas")
        self.assertEqual(self.refuse("diag", synthetic_diag(count=101)), "range:questions")
        for member, bad, detail in (
            ("signal", "blue", "range:areas[1].signal"),
            ("signal", 1, "type:areas[1].signal"),
            ("rating", "great", "range:areas[1].rating"),
            ("name", "n" * 81, "range:areas[1].name"),
            ("score", 101, "range:areas[1].score"),
        ):
            data = synthetic_diag()
            data["areas"][1][member] = bad
            self.assertEqual(self.refuse("diag", data), detail)
        for member, bad, detail in (
            ("no", 0, "range:questions[2].no"),
            ("no", 501, "range:questions[2].no"),
            ("correct", 1, "type:questions[2].correct"),
            ("correct_rate", float("nan"), "range:questions[2].correct_rate"),
            ("area", "", "range:questions[2].area"),
            ("difficulty", "d" * 33, "range:questions[2].difficulty"),
        ):
            data = synthetic_diag()
            data["questions"][2][member] = bad
            self.assertEqual(self.refuse("diag", data), detail)
        data = synthetic_diag()
        data["questions"][0]["unit"] = "x"
        self.assertEqual(self.refuse("diag", data), "unknown_member:questions[0]")
        data = synthetic_diag()
        data["difficulty"]["mid"]["questions"] = 501
        self.assertEqual(self.refuse("diag", data), "range:difficulty.mid.questions")
        data = synthetic_diag()
        del data["difficulty"]["low"]
        self.assertEqual(self.refuse("diag", data), "missing:difficulty.low")

    def test_r5_new_diag_adds_apply_code_and_mbti(self):
        out = validate_new_diag(synthetic_new_diag())
        self.assertEqual((out["apply_code"], out["mbti"]), ("Invented-Code-7", "INTP"))
        for code in MBTI_CODES:
            validate_new_diag(synthetic_new_diag(mbti=code))
        self.assertEqual(len(MBTI_CODES), 17)
        for bad, detail in (("intp", "range:mbti"), ("OTHER", "range:mbti"), ("XXXX", "range:mbti"), (7, "type:mbti")):
            self.assertEqual(self.refuse("new_diag", synthetic_new_diag(mbti=bad)), detail)
        self.assertEqual(self.refuse("new_diag", dict(synthetic_new_diag(), apply_code="")), "range:apply_code")
        self.assertEqual(self.refuse("new_diag", dict(synthetic_new_diag(), apply_code="a" * 65)), "range:apply_code")
        data = synthetic_new_diag()
        del data["apply_code"]
        self.assertEqual(self.refuse("new_diag", data), "missing:apply_code")
        self.assertEqual(self.refuse("new_diag", synthetic_diag()), "missing:apply_code")
        self.assertEqual(self.refuse("diag", synthetic_new_diag()), "unknown_member:input")

    def test_r6_formative_members_are_strict(self):
        out = validate_formative(synthetic_formative())
        self.assertEqual(set(out), {"title", "student", "test_date", "my_score", "avg_score", "rank_signal", "comment", "units", "difficulty", "questions"})
        validate_formative(synthetic_formative(units=12, count=100, comment=""))
        good = synthetic_formative()
        for name in good:
            missing = dict(good)
            del missing[name]
            self.assertEqual(self.refuse("formative", missing), "missing:" + name)
        for name, bad, detail in (("rank_signal", 0, "range:rank_signal"), ("rank_signal", 6, "range:rank_signal"), ("rank_signal", "3", "type:rank_signal"), ("my_score", 101, "range:my_score")):
            self.assertEqual(self.refuse("formative", dict(good, **{name: bad})), detail)
        self.assertEqual(self.refuse("formative", synthetic_formative(units=13)), "range:units")
        data = synthetic_formative()
        data["units"][0]["score"] = 1
        self.assertEqual(self.refuse("formative", data), "unknown_member:units[0]")
        data = synthetic_formative()
        data["questions"][0]["area"] = data["questions"][0].pop("unit")
        self.assertEqual(self.refuse("formative", data), "missing:questions[0].unit")
        self.assertEqual(self.refuse("formative", synthetic_diag()), "missing:my_score")
        self.assertEqual(self.refuse("diag", synthetic_formative()), "missing:grade")

    def test_r7_details_hold_no_input_value(self):
        value = "Zq" + "Xv" + "Learner-Name"
        for report_type in TYPES:
            data = SYNTHETIC[report_type]()
            data["student"] = value * 9
            detail = self.refuse(report_type, data)
            self.assertEqual(detail, "range:student")
            data = SYNTHETIC[report_type]()
            data[value] = value
            self.assertNotIn(value, self.refuse(report_type, data))
            data = SYNTHETIC[report_type]()
            data["questions"][0][value] = 1
            self.assertNotIn(value, self.refuse(report_type, data))
            data = SYNTHETIC[report_type]()
            data["title"] = {value: value}
            self.assertNotIn(value, self.refuse(report_type, data))


class Layout(unittest.TestCase):
    def test_r1_template_sequences(self):
        d, n, f = "image/diag/diag_%d.jpg", "image/new_diag/diag_%d.jpg", "image/formative/formative_%d.jpg"
        for count, extra in ((1, 0), (25, 0), (26, 1), (50, 1), (51, 2), (100, 3)):
            seq = [p.template for p in plan_of("diag", synthetic_diag(count=count)).pages]
            self.assertEqual(seq, [d % k for k in range(1, 10)] + [d % 9] * extra, count)
            self.assertEqual(seq, diag.template_sequence(count))
            seq = [p.template for p in plan_of("new_diag", synthetic_new_diag(count=count)).pages]
            self.assertEqual(seq, [n % k for k in range(1, 10)] + [n % 9] * extra + [n % 10], count)
            self.assertEqual(seq, new_diag.new_diag_template_sequence(count))
            seq = [p.template for p in plan_of("formative", synthetic_formative(count=count)).pages]
            self.assertEqual(seq, [f % k for k in range(1, 7)] + [f % 6] * extra, count)
            self.assertEqual(seq, formative.template_sequence(count))
        self.assertEqual(len(plan_of("diag", synthetic_diag()).pages), 9)
        self.assertEqual(len(plan_of("new_diag", synthetic_new_diag()).pages), 10)
        self.assertEqual(len(plan_of("formative", synthetic_formative()).pages), 6)

    def test_r2_required_assets_are_the_official_overlays(self):
        data = validate_diag(synthetic_diag())
        rels = diag.required_diag_assets(data)
        self.assertEqual(rels, sorted(set(rels)))
        for rel in ("image/diag/2p/lv04.jpg", "image/diag/4p/area_graph.jpg", "image/diag/4p/oneself-point.png", "image/diag/5p/light_green.jpg",
                    "image/diag/5p/light_yellow.jpg", "image/diag/6p/nice.jpg", "image/diag/8p/difficulty_chart.jpg", "image/mark_o.jpg", "image/mark_x.jpg"):
            self.assertIn(rel, rels)
        self.assertNotIn("image/diag/4p/03_graph.jpg", rels)
        rels = new_diag.required_new_diag_assets(validate_new_diag(synthetic_new_diag(mbti="ENFJ")))
        for rel in ("image/new_diag/diag_10.jpg", "image/diag/4p/03_graph.jpg", "image/diag/9p/ENFJ.jpg", "image/diag/6p/fail.jpg", "image/diag/2p/lv04.jpg"):
            self.assertIn(rel, rels)
        self.assertFalse([r for r in rels if "/5p/" in r or "/8p/" in r or r.startswith("image/diag/diag_")])
        self.assertIn("image/diag/9p/other.jpg", new_diag.required_new_diag_assets(validate_new_diag(synthetic_new_diag(mbti="other"))))
        rels = formative.required_formative_assets(validate_formative(synthetic_formative()))
        for rel in ("image/formative/2p/score_chart.jpg", "image/formative/4p/03 graph.jpg", "image/formative/4p/03 average-point.png",
                    "image/formative/4p/03 oneself-point.png", "image/formative/6p/signal_3.jpg", "image/formative/formative_6.jpg"):
            self.assertIn(rel, rels)
        # Every picture a plan draws is a required asset.
        for report_type in TYPES:
            data = VALIDATE[report_type](SYNTHETIC[report_type](count=60))
            plan = PLAN[report_type](data, FAKE_CTX)
            used = {p.template for p in plan.pages} | {i.rel for p in plan.pages for i in p.images}
            self.assertEqual(used - set(REQUIRED[report_type](data)), set(), report_type)

    def test_r3_fields_fit_their_boxes_inside_a4_without_overlap(self):
        for report_type in TYPES:
            for kwargs in ({}, {"count": 100}, {"count": 1, "comment": ""}):
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
            data["title"] = "T" * 120
            data["questions"][0]["difficulty"] = "D" * 32
            plan = plan_of(report_type, data)
            title = next(f for f in plan.pages[0].fields if f.id == "p1.title")
            self.assertEqual(title.size, 6.0)
            self.assertTrue(title.text.endswith("..."))
            cell = next(f for f in all_fields(plan) if f.id.endswith(".q0.difficulty"))
            self.assertEqual(cell.size, 6.0)
            self.assertTrue(cell.text.endswith("..."))

    def test_r3_comment_is_wrapped_and_cut_with_an_ellipsis(self):
        for report_type, page_no in (("diag", 2), ("new_diag", 2), ("formative", 1)):
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
            data["title"] = "Invented\\r\\ntitle"
            data["questions"][0]["difficulty"] = "Hi\\ngh"
            plan = plan_of(report_type, data)
            fields = all_fields(plan)
            comment = [f.text for f in fields if ".comment." in f.id]
            self.assertEqual(comment, ["alpha", "beta", "gamma", "delta", "epsilon"])
            self.assertEqual(next(f.text for f in fields if f.id == "p1.title"), "Invented title")
            for f in fields:
                self.assertNotIn("\\", f.text)
                self.assertNotIn("\n", f.text)
                self.assertNotIn("rn", f.text.split())

    def test_r3_question_rows_columns_and_marks(self):
        plan = plan_of("diag", synthetic_diag(count=27))
        first, second = plan.pages[8], plan.pages[9]
        px = core.geometry.px
        by_id = {f.id: f for f in first.fields}
        self.assertEqual(by_id["p9.q0.no"].box, (px(247), px(334), px(425), px(432)))
        self.assertEqual(by_id["p9.q24.area"].box[1], px(334 + 24 * 98))
        self.assertEqual(by_id["p9.q1.area"].align, "l")
        self.assertEqual(by_id["p9.q3.correct_rate"].text, "11.5")
        self.assertEqual([i.rel for i in first.images][:3], ["image/mark_x.jpg", "image/mark_o.jpg", "image/mark_o.jpg"])
        self.assertEqual(len(first.images), 25)
        self.assertEqual(sorted(f.id for f in second.fields if f.id.endswith(".no")), ["p10.q25.no", "p10.q26.no"])
        self.assertLessEqual(334 + 25 * 98, 3290)
        plan = plan_of("formative", synthetic_formative(count=27))
        first, second = plan.pages[5], plan.pages[6]
        by_id = {f.id: f for f in first.fields}
        self.assertEqual(by_id["p6.q0.no"].box[:2], (px(212), px(1794)))
        self.assertEqual(by_id["p6.q14.no"].box[1], px(1794 + 14 * 98))
        self.assertEqual(by_id["p6.q15.no"].box[:2], (px(212 + 1104), px(1794)))
        self.assertEqual(by_id["p6.q24.no"].box[1], px(1794 + 9 * 98))
        self.assertEqual(first.images[0].rel, "image/formative/6p/signal_3.jpg")
        self.assertEqual(len(first.images), 26)
        self.assertNotIn("image/formative/6p/signal_3.jpg", [i.rel for i in second.images])
        self.assertEqual(sorted(f.id for f in second.fields if f.id.endswith(".no")), ["p7.q25.no", "p7.q26.no"])
        # The formative table has no unit column: the unit is not printed.
        self.assertFalse([f for f in all_fields(plan) if f.text.startswith("Unit") and f.id.startswith(("p6.", "p7."))])

    def test_r3_area_and_unit_counts_come_from_the_questions_only(self):
        data = validate_diag(synthetic_diag(areas=6, count=20))
        self.assertEqual(diag.area_counts(data), [(3, 5), (4, 5), (3, 5), (3, 5), (0, 0), (0, 0)])
        plan = diag.plan_diag(data, FAKE_CTX)
        table = {f.id: f.text for f in plan.pages[5].fields}
        self.assertEqual([table["p6.a%d.name" % i] for i in range(6)], ["Area %d" % (i + 1) for i in range(6)])
        self.assertEqual([table["p6.a%d.total" % i] for i in range(6)], ["5", "5", "5", "5", "0", "0"])
        self.assertEqual(len([k for k in table if k.endswith(".name")]), 6)  # one row an area, none invented
        self.assertEqual(plan.pages[5].shapes[0].kind, "mask")
        lights = [i.rel for i in plan.pages[5].images if "/5p/" in i.rel]
        self.assertEqual(lights, [diag.light_rel(a["signal"]) for a in data["areas"]])
        cards = [i.rel for i in plan.pages[5].images if "/6p/" in i.rel]
        self.assertEqual(cards, [diag.rating_rel(a["rating"]) for a in data["areas"]])
        graph = plan.pages[4]
        self.assertEqual([i.rel for i in graph.images], ["image/diag/4p/area_graph.jpg"] + [diag.POINT] * 4)
        self.assertEqual(sorted(f.id for f in graph.fields), ["p5.a%d.questions" % i for i in range(4)])
        new = new_diag.plan_new_diag(validate_new_diag(synthetic_new_diag(areas=6)), FAKE_CTX)
        self.assertFalse([i for p in new.pages for i in p.images if "/5p/" in i.rel or "/8p/" in i.rel])
        self.assertEqual(new.pages[-1].images[0].rel, "image/diag/9p/INTP.jpg")
        self.assertFalse([f for f in all_fields(new) if "Invented-Code" in f.text])
        data = validate_formative(synthetic_formative(units=6, count=20))
        self.assertEqual(formative.unit_counts(data), [5, 5, 5, 5, 0, 0])
        plan = formative.plan_formative(data, FAKE_CTX)
        self.assertEqual(sorted(f.id for f in plan.pages[2].fields if f.id.startswith("p3.u")), ["p3.u%d.questions" % i for i in range(4)])
        self.assertEqual(len([f for f in plan.pages[3].fields if f.id.endswith(".name")]), 6)
        self.assertEqual([i.rel for i in plan.pages[3].images][1:3], [formative.AVERAGE_POINT, formative.ONESELF_POINT])

    def test_r3_scores_marks_and_bars_follow_the_input(self):
        px = core.geometry.px
        plan = plan_of("diag", synthetic_diag())
        summary = {f.id: f for f in plan.pages[2].fields}
        self.assertEqual((summary["p3.total_score"].text, summary["p3.avg_score"].text), ("72.5", "61"))
        bars = [s for s in plan.pages[2].shapes if s.kind == "rect"]
        self.assertAlmostEqual(bars[0].geometry[2] - bars[0].geometry[0], px(diag.BAR_PX_PER_POINT * 72.5))
        self.assertEqual(plan.pages[2].images[0].rel, "image/diag/2p/lv04.jpg")
        difficulty = {f.id: f.text for f in plan.pages[3].fields}
        self.assertEqual([difficulty["p4.%s.questions" % lv] for lv in common.LEVELS], ["2", "5", "8", "5"])
        self.assertEqual(difficulty["p4.high.score"], "55.5")
        self.assertEqual(len([s for s in plan.pages[3].shapes if s.kind == "poly"]), 4)
        plan = plan_of("formative", synthetic_formative())
        results = {f.id: f.text for f in plan.pages[1].fields}
        self.assertEqual((results["p2.rank_signal"], results["p2.my_score"], results["p2.avg_score"]), ("3", "80", "66.5"))
        self.assertEqual({f.id: f.text for f in plan.pages[2].fields}["p3.questions"], "20")
        level_bars = [s for s in plan.pages[3].shapes if s.kind == "rect"]
        self.assertEqual(len(level_bars), 4)
        self.assertAlmostEqual(level_bars[3].geometry[3] - level_bars[3].geometry[1], px(formative.LEVEL_PX_PER_POINT * 100))
        zero = synthetic_formative()
        zero["difficulty"]["low"]["score"] = 0
        self.assertEqual(len([s for s in plan_of("formative", zero).pages[3].shapes if s.kind == "rect"]), 3)

    def test_r8_the_plan_is_deterministic(self):
        for report_type in TYPES:
            self.assertEqual(plan_of(report_type, SYNTHETIC[report_type](count=60)), plan_of(report_type, SYNTHETIC[report_type](count=60)))


class Command(core.TempDirCase):
    def test_r1_types_are_known_and_refuse_the_other_types_inputs(self):
        for report_type in TYPES:
            status, line = core.run_cli(["render", "--type", report_type, "--assets", self.tmp, "--out", self.path("o.pdf")], b"{}")
            self.assertEqual((status, json.loads(line)["code"]), (1, "input_invalid"))
            status, line = core.run_cli(["render", "--type", report_type, "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(SYNTHETIC[report_type]()).encode())
            self.assertEqual(json.loads(line)["code"], "asset_manifest_invalid")
            for other in TYPES + ("course_book",):
                if other == report_type:
                    continue
                source = core.synthetic_input() if other == "course_book" else SYNTHETIC[other]()
                status, line = core.run_cli(["render", "--type", report_type, "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(source).encode())
                self.assertEqual(json.loads(line)["code"], "input_invalid", (report_type, other))
        self.assertEqual(os.listdir(self.tmp), [])

    def test_r10_parity_module_takes_the_new_types(self):
        from studyq_linux import parity

        out = io.StringIO()
        saved, sys.stdout = sys.stdout, out
        try:
            for report_type in TYPES:
                self.assertEqual(parity.main(["--pdf", self.path("absent.pdf"), "--assets", self.tmp, "--type", report_type]), 1)
            self.assertEqual(parity.main(["--pdf", "p", "--assets", "a", "--type", "smart_book"]), 2)
        finally:
            sys.stdout = saved
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
        for report_type, base in (("diag", 9), ("new_diag", 10), ("formative", 6)):
            for count, extra in ((20, 0), (26, 1)):
                status, doc, _ = self.render(report_type, SYNTHETIC[report_type](count=count), self.path("%s-%d.pdf" % (report_type, count)))
                self.assertEqual(status, 0, doc)
                self.assertEqual(list(doc), ["status", "reportType", "pages", "sha256", "outputPath"])
                self.assertEqual((doc["reportType"], doc["pages"]), (report_type, base + extra))

    def test_r10_parity_reports_zero_mismatches_and_zero_raw_rn(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context

        for report_type in TYPES:
            data = SYNTHETIC[report_type](count=60, comment="Invented comment.\\r\\nSecond line of it.\n" + "word " * 300)
            if report_type != "formative":
                data["areas"] = synthetic_diag(areas=8)["areas"]
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

        for report_type, rel in (("diag", "image/diag/5p/light_red.jpg"), ("new_diag", "image/diag/9p/INTP.jpg"), ("formative", "image/formative/6p/signal_3.jpg")):
            contents = report_contents()
            del contents[rel]
            data = SYNTHETIC[report_type]()
            if report_type == "diag":
                data["areas"][0]["signal"] = "red"
            with self.assertRaises(StudyqError) as caught:
                render_report(report_type, data, manifest.AssetBundle.from_mapping(contents))
            self.assertEqual(caught.exception.code, "asset_missing")


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
