"""Tests of the `cism` rules of specs/studyq-linux/core.md (R18, R19). Synthetic fixtures, invented values only.

The pure-Python tests (input schema, layout and pagination of the plan, the command's words) run
unconditionally (R26); the rendering and parity tests run under the gate of R27, and the parity
check against the official templates only when `STUDYQ_ASSETS_DIR` names an assets directory.
"""

import io
import json
import os
import subprocess
import sys
import unittest

import test_core as core
from studyq_linux import cli, manifest
from studyq_linux.course_book import PlanContext
from studyq_linux.errors import StudyqError
from studyq_linux.renderers import cism
from studyq_linux.schema import validate_cism

BANNER_SIZES = {
    "4_1": (330, 47),
    "4_2": (396, 47),
    "5_1": (319, 47),
    "5_2": (352, 47),
    "5_3": (318, 47),
    "5_4": (431, 47),
    "6_1": (405, 47),
    "6_2": (386, 47),
}
IMAGE_SIZES = {cism.TEMPLATE_DIR + name + ".jpg": size for name, size in BANNER_SIZES.items()}
IMAGE_SIZES.update({cism.PAGE1: (2481, 3508), cism.OVERFLOW: (2481, 3509), cism.NARRATIVE: (2481, 3508)})
FAKE_CTX = PlanContext(lambda text, size: len(text) * size * 0.6, lambda rel: IMAGE_SIZES[rel])
ALL_SECTIONS = [(4, 1), (4, 2), (5, 1), (5, 2), (5, 3), (5, 4), (6, 1), (6, 2)]


def synthetic_cism(rows=6, sections=ALL_SECTIONS, text="Sample narrative line one.\\r\\nSecond line, invented."):
    return {
        "week_no": 7,
        "week_label": "2040-7 week (02.10 ~ 02.16)",
        "teacher": "Sample Teacher",
        "period": "02.10-02.16",
        "chart_data": [
            {
                "group": "Group %d" % (i % 3 + 1),
                "student": "Learner %d" % (i + 1),
                "class_mode": ["", "A+", "A-", "B+"][i % 4],
                "think_note": "" if i % 5 == 0 else "%d%%" % (50 + i % 50),
                "week_test": "" if i % 7 == 0 else "%d pt" % (40 + i % 60),
            }
            for i in range(rows)
        ],
        "report_list": [{"group": g, "slot": s, "title": "Section %d-%d" % (g, s), "text": text} for g, s in sections],
    }


def plan_of(data):
    return cism.plan_cism(validate_cism(data), FAKE_CTX)


def narrative_pages(plan):
    return [p for p in plan.pages if p.template == cism.NARRATIVE]


class Schema(unittest.TestCase):
    def refuse(self, data):
        with self.assertRaises(StudyqError) as caught:
            validate_cism(data)
        self.assertEqual(caught.exception.code, "input_invalid")
        return caught.exception.detail

    def test_r19_good_input_is_accepted_and_normalised(self):
        out = validate_cism(synthetic_cism())
        self.assertEqual(set(out), {"week_no", "week_label", "teacher", "period", "chart_data", "report_list"})
        self.assertEqual(out["report_list"][0]["text"], "Sample narrative line one.\nSecond line, invented.")
        self.assertEqual(validate_cism(synthetic_cism(rows=0))["chart_data"], [])
        self.assertEqual(len(validate_cism(synthetic_cism(rows=1200))["chart_data"]), 1200)
        validate_cism(synthetic_cism(sections=[(4, 1)], text=""))
        validate_cism(synthetic_cism(sections=[(4, 1)], text="x" * 40000))

    def test_r19_members_are_strict(self):
        good = synthetic_cism()
        missing = dict(good)
        del missing["period"]
        self.assertEqual(self.refuse(missing), "missing:period")
        self.assertEqual(self.refuse(dict(good, surplus=1)), "unknown_member:input")
        self.assertEqual(self.refuse(dict(good, week_no="7")), "type:week_no")
        self.assertEqual(self.refuse(dict(good, week_no=True)), "type:week_no")
        self.assertEqual(self.refuse(dict(good, week_no=0)), "range:week_no")
        self.assertEqual(self.refuse(dict(good, week_no=54)), "range:week_no")
        validate_cism(dict(good, week_no=1))
        validate_cism(dict(good, week_no=53))
        for name in ("week_label", "teacher", "period"):
            self.assertEqual(self.refuse(dict(good, **{name: ""})), "range:" + name)
            self.assertEqual(self.refuse(dict(good, **{name: " \n "})), "range:" + name)
            self.assertEqual(self.refuse(dict(good, **{name: 5})), "type:" + name)
        self.assertEqual(self.refuse(dict(good, teacher="t" * 65)), "range:teacher")
        self.assertEqual(self.refuse(dict(good, week_label="w" * 121)), "range:week_label")
        for name, limit in (("group", 120), ("student", 64), ("class_mode", 16), ("think_note", 32), ("week_test", 32)):
            data = synthetic_cism()
            data["chart_data"][2][name] = "x" * (limit + 1)
            self.assertEqual(self.refuse(data), "range:chart_data[2].%s" % name)
        data = synthetic_cism()
        data["chart_data"][1]["group"] = ""
        self.assertEqual(self.refuse(data), "range:chart_data[1].group")
        data = synthetic_cism()
        del data["chart_data"][0]["week_test"]
        self.assertEqual(self.refuse(data), "missing:chart_data[0].week_test")
        data = synthetic_cism()
        data["chart_data"][0]["extra"] = "x"
        self.assertEqual(self.refuse(data), "unknown_member:chart_data[0]")
        data = synthetic_cism()
        data["chart_data"][0]["student"] = 3
        self.assertEqual(self.refuse(data), "type:chart_data[0].student")
        self.assertEqual(self.refuse(synthetic_cism(rows=1201)), "range:chart_data")

    def test_r19_report_list_slots_and_duplicates(self):
        for group, slot in ((4, 3), (4, 0), (5, 5), (6, 3), (3, 1), (7, 1)):
            data = synthetic_cism(sections=[(group, slot)])
            self.assertTrue(self.refuse(data).startswith("range:report_list[0]."), (group, slot))
        self.assertEqual(self.refuse(synthetic_cism(sections=[(4, 1), (5, 2), (4, 1)])), "duplicate:report_list[2].slot")
        self.assertEqual(self.refuse(synthetic_cism(sections=[])), "range:report_list")
        crowded = synthetic_cism()
        crowded["report_list"] = crowded["report_list"] + [dict(crowded["report_list"][0])]
        self.assertEqual(self.refuse(crowded), "range:report_list")
        data = synthetic_cism(sections=[(4, 1)])
        data["report_list"][0]["group"] = "4"
        self.assertEqual(self.refuse(data), "type:report_list[0].group")
        data["report_list"][0]["group"] = True
        self.assertEqual(self.refuse(data), "type:report_list[0].group")
        data = synthetic_cism(sections=[(4, 1)])
        data["report_list"][0]["title"] = ""
        self.assertEqual(self.refuse(data), "range:report_list[0].title")
        data["report_list"][0]["title"] = "t"
        data["report_list"][0]["text"] = "x" * 40001
        self.assertEqual(self.refuse(data), "range:report_list[0].text")
        del data["report_list"][0]["text"]
        self.assertEqual(self.refuse(data), "missing:report_list[0].text")

    def test_r7_cism_details_hold_no_input_value(self):
        value = "Zq" + "Xv" + "Student-Name"
        data = synthetic_cism()
        data["chart_data"][0]["student"] = value * 8
        data["report_list"][0]["title"] = value
        data[value] = 1
        self.assertNotIn(value, self.refuse(data))
        data = synthetic_cism()
        data["chart_data"][0]["student"] = value * 8
        self.assertNotIn(value, self.refuse(data))
        data = synthetic_cism()
        data["teacher"] = {value: 1}
        self.assertNotIn(value, self.refuse(data))


class Layout(unittest.TestCase):
    def test_r18_template_sequence_follows_the_row_count(self):
        def templates(rows):
            return [p.template for p in plan_of(synthetic_cism(rows=rows, sections=[(4, 1)])).pages]

        n1, n2, n3 = cism.PAGE1, cism.OVERFLOW, cism.NARRATIVE
        self.assertEqual(templates(0), [n1, n3])
        self.assertEqual(templates(31), [n1, n3])
        self.assertEqual(templates(32), [n1, n2, n3])
        self.assertEqual(templates(69), [n1, n2, n3])
        self.assertEqual(templates(70), [n1, n2, n2, n3])
        self.assertEqual(templates(1200), [n1] + [n2] * 31 + [n3])
        self.assertEqual(cism.chart_rows_per_page(1200), [31] + [38] * 30 + [29])

    def test_r18_required_assets(self):
        base = validate_cism(synthetic_cism(rows=3, sections=[(4, 2), (6, 1)]))
        self.assertEqual(cism.required_cism_assets(base), sorted([cism.PAGE1, cism.NARRATIVE, cism.TEMPLATE_DIR + "4_2.jpg", cism.TEMPLATE_DIR + "6_1.jpg"]))
        more = validate_cism(synthetic_cism(rows=32, sections=[(4, 2)]))
        self.assertIn(cism.OVERFLOW, cism.required_cism_assets(more))
        everything = validate_cism(synthetic_cism())
        self.assertEqual(len(cism.required_cism_assets(everything)), 2 + len(ALL_SECTIONS))
        for rel in cism.required_cism_assets(everything):
            self.assertIn(rel, IMAGE_SIZES)

    def test_r19_header_fields_sit_in_their_boxes_without_overlap(self):
        page = plan_of(synthetic_cism()).pages[0]
        header = {f.id: f for f in page.fields if f.id in ("p1.week_label", "p1.teacher", "p1.period")}
        self.assertEqual(set(header), {"p1.week_label", "p1.teacher", "p1.period"})
        self.assertEqual(header["p1.week_label"].box, (46.79, 146.4, 335.0, 165.6))
        self.assertEqual(header["p1.teacher"].box, (343.1, 146.4, 467.9, 165.6))
        self.assertEqual(header["p1.period"].box, (467.9, 146.4, 545.9, 165.6))
        self.assertEqual([header[k].align for k in ("p1.week_label", "p1.teacher", "p1.period")], ["l", "l", "r"])
        self.assertTrue(all(f.size == 11.0 for f in header.values()))
        boxes = sorted(f.box for f in header.values())
        for a, b in zip(boxes, boxes[1:]):
            self.assertLessEqual(a[2], b[0])

    def test_r19_long_header_values_shrink_then_are_cut(self):
        data = synthetic_cism()
        data["period"] = "P" * 60
        page = plan_of(data).pages[0]
        period = next(f for f in page.fields if f.id == "p1.period")
        self.assertEqual(period.size, 6.0)
        self.assertTrue(period.text.endswith("..."))
        self.assertLessEqual(FAKE_CTX.measure(period.text, period.size), period.box[2] - period.box[0] - 2)

    def test_r19_chart_rows_columns_and_no_field_for_empty_cells(self):
        data = synthetic_cism(rows=31)
        page = plan_of(data).pages[0]
        by_id = {f.id: f for f in page.fields}
        px = core.geometry.px
        self.assertEqual(by_id["p1.r0.group"].box, (px(195), px(920), px(657), px(995)))
        self.assertEqual(by_id["p1.r30.week_test"].box, (px(1876), px(920 + 30 * 75), px(2283), px(920 + 31 * 75)))
        self.assertLessEqual(920 + 31 * 75, 3250)
        spans = {n: (x0, x1) for n, x0, x1, _ in cism.CHART_COLUMNS_PX}
        self.assertEqual(spans, {"group": (195, 657), "student": (657, 1063), "class_mode": (1063, 1469), "think_note": (1469, 1876), "week_test": (1876, 2283)})
        self.assertTrue(all(f.size <= 10.5 for f in page.fields if ".r" in f.id))
        for i, row in enumerate(data["chart_data"]):
            for name in ("class_mode", "think_note", "week_test"):
                self.assertEqual("p1.r%d.%s" % (i, name) in by_id, row[name] != "", (i, name))
        self.assertTrue(all(f.text for p in plan_of(data).pages for f in p.fields))
        self.assertIn("p1.r1.class_mode", by_id)
        self.assertNotIn("p1.r0.class_mode", by_id)

    def test_r18_overflow_pages_hold_38_rows_from_360_px(self):
        plan = plan_of(synthetic_cism(rows=31 + 38 + 5, sections=[(4, 1)]))
        self.assertEqual([p.template for p in plan.pages], [cism.PAGE1, cism.OVERFLOW, cism.OVERFLOW, cism.NARRATIVE])
        px = core.geometry.px
        second = {f.id: f for f in plan.pages[1].fields}
        self.assertEqual(second["p2.r31.group"].box, (px(195), px(360), px(657), px(435)))
        self.assertEqual(second["p2.r68.group"].box[3], px(360 + 38 * 75))
        self.assertLessEqual(360 + 38 * 75, 3250)
        third = {f.id for f in plan.pages[2].fields}
        self.assertIn("p3.r69.group", third)
        self.assertEqual(len([i for i in third if i.endswith(".group")]), 5)

    def test_r18_every_student_row_is_planned_once(self):
        rows = 200
        plan = plan_of(synthetic_cism(rows=rows, sections=[(4, 1)]))
        groups = [f.id for p in plan.pages for f in p.fields if f.id.endswith(".group")]
        self.assertEqual(sorted(int(i.split(".r")[1].split(".")[0]) for i in groups), list(range(rows)))

    def test_r18_narrative_sections_follow_group_and_slot_order_with_their_banners(self):
        shuffled = [(6, 2), (4, 2), (5, 3), (4, 1), (6, 1), (5, 1)]
        plan = plan_of(synthetic_cism(sections=shuffled))
        pages = narrative_pages(plan)
        self.assertEqual(len(pages), 1)
        banners = [i.rel for i in pages[0].images]
        self.assertEqual(banners, [cism.TEMPLATE_DIR + n + ".jpg" for n in ("4_1", "4_2", "5_1", "5_3", "6_1", "6_2")])
        self.assertEqual([i.rect[0] for i in pages[0].images], [46.79] * 6)
        self.assertAlmostEqual(pages[0].images[0].rect[1], 120.2)
        w = core.geometry.px(330)
        self.assertAlmostEqual(pages[0].images[0].rect[2] - 46.79, w)
        self.assertAlmostEqual(pages[0].images[0].rect[3] - 120.2, core.geometry.px(47))
        ys = [i.rect[1] for i in pages[0].images]
        self.assertEqual(ys, sorted(ys))
        for page in pages:
            for image in page.images:
                self.assertLessEqual(image.rect[3], cism.BOTTOM)

    def test_r18_text_lines_sit_inside_their_frame_in_the_text_area(self):
        plan = plan_of(synthetic_cism(sections=[(4, 1)]))
        page = narrative_pages(plan)[0]
        frame = page.shapes[0]
        self.assertEqual(frame.kind, "frame")
        self.assertEqual((frame.geometry[0], frame.geometry[2]), (46.79, 547.77))
        banner = page.images[0].rect
        self.assertAlmostEqual(frame.geometry[1], banner[3] + 6.0)
        lines = [f for f in page.fields if f.id.startswith("cism.s4_1.")]
        self.assertEqual([f.text for f in lines], ["Sample narrative line one.", "Second line, invented."])
        self.assertEqual([f.id for f in lines], ["cism.s4_1.l0", "cism.s4_1.l1"])
        self.assertAlmostEqual(frame.geometry[3] - frame.geometry[1], 2 * 6.0 + 2 * 16.8)
        for k, f in enumerate(lines):
            self.assertAlmostEqual(f.box[1], frame.geometry[1] + 6.0 + k * 16.8)
            self.assertAlmostEqual(f.box[3] - f.box[1], 16.8)
            self.assertEqual((f.box[0], f.box[2], f.size, f.align), (46.79 + 6.0, 547.77 - 6.0, 10.5, "l"))

    def test_r16_line_break_tokens_break_and_are_not_printed_in_a_section(self):
        text = "alpha\\r\\nbeta\\\\ngamma\r\ndelta\nepsilon"
        page = narrative_pages(plan_of(synthetic_cism(sections=[(5, 1)], text=text)))[0]
        self.assertEqual([f.text for f in page.fields], ["alpha", "beta", "gamma", "delta", "epsilon"])
        for f in page.fields:
            self.assertNotIn("\\", f.text)
            self.assertNotIn("rn", f.text.split())

    def test_r18_an_empty_section_is_an_empty_frame_with_no_field(self):
        plan = plan_of(synthetic_cism(rows=0, sections=[(4, 1), (4, 2)], text=""))
        page = narrative_pages(plan)[0]
        self.assertEqual(page.fields, [])
        self.assertEqual(len(page.shapes), 2)
        for frame in page.shapes:
            self.assertAlmostEqual(frame.geometry[3] - frame.geometry[1], 2 * 6.0 + 16.8)
        self.assertAlmostEqual(page.shapes[1].geometry[1] - page.images[1].rect[3], 6.0)
        self.assertAlmostEqual(page.images[1].rect[1] - page.shapes[0].geometry[3], 12.0)

    def test_r18_long_text_continues_on_new_pages_without_a_repeated_banner_or_a_lost_line(self):
        text = "\n".join("Invented line number %d of a very long narrative." % i for i in range(120))
        plan = plan_of(synthetic_cism(rows=0, sections=[(5, 2)], text=text))
        pages = narrative_pages(plan)
        self.assertEqual(len(pages), 4)  # 39 lines fit under the banner on page 1 less its height, then 39 per page
        self.assertEqual(len([i for p in pages for i in p.images]), 1)
        self.assertEqual(len(pages[0].images), 1)
        shown = [f.text for p in pages for f in p.fields]
        self.assertEqual(shown, text.split("\n"))
        for p in pages[1:]:
            self.assertEqual(p.images, [])
            self.assertAlmostEqual(p.shapes[0].geometry[1], 120.2)
        for p in pages:
            for frame in p.shapes:
                self.assertLessEqual(frame.geometry[3], 792.0 + 1e-6)
            for f in p.fields:
                self.assertGreaterEqual(f.box[1], 120.2)
                self.assertLessEqual(f.box[3], 792.0 + 1e-6)
        self.assertEqual([f.id for p in pages for f in p.fields], ["cism.s5_2.l%d" % i for i in range(120)])

    def test_r18_a_section_that_cannot_start_above_the_bottom_starts_a_fresh_page(self):
        # Fill page 1 until less than banner + gap + one framed line remains, then add a section.
        usable = cism.BOTTOM - cism.TOP
        banner = core.geometry.px(47)
        lines = int((usable - banner - cism.BANNER_GAP - 2 * cism.BOX_PAD) // cism.LINE_H) - 1
        first = "\n".join("l%d" % i for i in range(lines))
        data = synthetic_cism(rows=0, sections=[(4, 1), (4, 2)], text=first)
        data["report_list"][1]["text"] = "short"
        pages = narrative_pages(plan_of(data))
        used = banner + cism.BANNER_GAP + 2 * cism.BOX_PAD + lines * cism.LINE_H + cism.SECTION_GAP
        remaining = usable - used
        self.assertLess(remaining, banner + cism.BANNER_GAP + 2 * cism.BOX_PAD + cism.LINE_H)
        self.assertEqual(len(pages), 2)
        self.assertEqual([len(p.images) for p in pages], [1, 1])
        self.assertAlmostEqual(pages[1].images[0].rect[1], 120.2)
        # One line fewer leaves room, and the second section stays on the page.
        data = synthetic_cism(rows=0, sections=[(4, 1), (4, 2)], text="\n".join("l%d" % i for i in range(lines - 4)))
        data["report_list"][1]["text"] = "short"
        self.assertEqual(len(narrative_pages(plan_of(data))), 1)

    def test_r18_the_largest_valid_input_terminates_with_no_truncation(self):
        text = "\n".join(["Invented sentence %d of the longest narrative allowed." % i for i in range(700)])[:40000]
        data = synthetic_cism(rows=1200, text=text)
        plan = plan_of(data)
        shown = {}
        for p in narrative_pages(plan):
            for f in p.fields:
                section, index = f.id.split(".")[1], int(f.id.split(".")[2][1:])
                shown.setdefault(section, []).append((index, f.text))
        for g, s in ALL_SECTIONS:
            got = shown["s%d_%d" % (g, s)]
            self.assertEqual([i for i, _ in got], list(range(len(got))))
        self.assertEqual(len(plan.pages), 1 + 31 + len(narrative_pages(plan)))
        self.assertEqual(sum(1 for p in plan.pages for f in p.fields if f.id.startswith("p")), sum(len([v for k, v in r.items() if v]) for r in data["chart_data"]) + 3)
        width = 500.98 - 12.0 - 2.0
        for p in narrative_pages(plan):
            for f in p.fields:
                self.assertLessEqual(FAKE_CTX.measure(f.text, f.size), width + 1e-9)

    def test_r8_the_plan_is_deterministic(self):
        self.assertEqual(plan_of(synthetic_cism(rows=40)), plan_of(synthetic_cism(rows=40)))


class Command(core.TempDirCase):
    def test_interim_cism_is_no_longer_not_implemented(self):
        status, line = core.run_cli(["render", "--type", "cism", "--assets", self.tmp, "--out", self.path("o.pdf")], b"{}")
        doc = json.loads(line)
        self.assertEqual((status, doc["code"], doc["detail"]), (1, "input_invalid", "missing:week_no"))
        status, line = core.run_cli(["render", "--type", "cism", "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(synthetic_cism()).encode())
        self.assertEqual(json.loads(line)["code"], "asset_manifest_invalid")
        self.assertEqual(os.listdir(self.tmp), [])

    def test_r19_cism_refuses_the_course_book_members_and_the_reverse(self):
        status, line = core.run_cli(["render", "--type", "cism", "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(core.synthetic_input()).encode())
        self.assertEqual(json.loads(line)["code"], "input_invalid")
        status, line = core.run_cli(["render", "--type", "course_book", "--assets", self.tmp, "--out", self.path("o.pdf")], json.dumps(synthetic_cism()).encode())
        self.assertEqual(json.loads(line)["code"], "input_invalid")

    def test_parity_module_usage_words(self):
        from studyq_linux import parity

        out = io.StringIO()
        for argv in (["--pdf", "p"], ["--pdf", "p", "--assets", "a", "--type", "other"], ["--pdf", "p", "--assets", "a", "--type"], ["--pdf", "p", "--assets", "a", "--assets", "b"], ["--pdf", "p", "--assets", "a", "--out", "x"]):
            saved, sys.stdout = sys.stdout, out
            try:
                self.assertEqual(parity.main(argv), 2, argv)
            finally:
                sys.stdout = saved
        self.assertEqual({json.loads(l)["code"] for l in out.getvalue().splitlines()}, {"usage_invalid"})


def cism_contents():
    contents = {manifest.FONT_REL: core.font_bytes()}
    for rel, (w, h) in IMAGE_SIZES.items():
        contents[rel] = core.picture(248 if w > 1000 else w, 351 if w > 1000 else h, "jpg")
    return contents


@core.needs_pymupdf
class Rendering(core.TempDirCase):
    def assets(self):
        directory = self.path("assets")
        os.makedirs(directory, exist_ok=True)
        core.write_assets(directory, cism_contents())
        return directory

    def render(self, data, out=None):
        out = out or self.path("out.pdf")
        status, line = core.run_cli(["render", "--type", "cism", "--assets", self.assets(), "--out", out], json.dumps(data).encode())
        return status, json.loads(line), out

    def parity(self, data, out):
        env = dict(os.environ, PYTHONPATH=os.path.join(core.MODULE, "src"))
        proc = subprocess.run(
            [sys.executable, "-B", "-m", "studyq_linux.parity", "--pdf", out, "--assets", self.assets(), "--type", "cism"],
            input=json.dumps(data).encode(),
            env=env,
            capture_output=True,
        )
        return proc.returncode, json.loads(proc.stdout)

    def test_r1_cism_success_line_and_page_count(self):
        status, doc, out = self.render(synthetic_cism())
        self.assertEqual(status, 0, doc)
        self.assertEqual(list(doc), ["status", "reportType", "pages", "sha256", "outputPath"])
        self.assertEqual((doc["reportType"], doc["pages"]), ("cism", 2))
        # One section of 60 lines: page 1, one overflow page (40 rows), then 38 + 22 lines on 2 narrative pages.
        big = synthetic_cism(rows=40, sections=[(4, 1)], text="\n".join("line %d" % i for i in range(60)))
        status, doc, _ = self.render(big, self.path("big.pdf"))
        self.assertEqual((status, doc["pages"]), (0, 1 + 1 + 2))

    def test_r18_cism_pages_use_the_verified_template_sequence(self):
        for rows, pages in ((0, 2), (31, 2), (32, 3), (70, 4)):
            status, doc, _ = self.render(synthetic_cism(rows=rows, sections=[(4, 1)]), self.path("seq%d.pdf" % rows))
            self.assertEqual((status, doc["pages"]), (0, pages), rows)

    def test_r8_cism_same_input_same_bytes(self):
        _, first, out1 = self.render(synthetic_cism(), self.path("a.pdf"))
        _, second, _ = self.render(synthetic_cism(), self.path("b.pdf"))
        self.assertEqual(first["sha256"], second["sha256"])
        with open(out1, "rb") as handle:
            raw = handle.read()
        self.assertNotIn(b"CreationDate", raw)
        self.assertNotIn(b"/ID", raw)

    def test_r12_cism_embeds_only_nanum_gothic(self):
        _, _, out = self.render(synthetic_cism())
        for page in core.pymupdf.open(out):
            self.assertEqual({f[3] for f in page.get_fonts()}, {"NanumGothic Regular"})

    def test_r19_parity_reports_zero_mismatches_and_zero_raw_rn(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context

        data = synthetic_cism(rows=75, text="Invented narrative.\\r\\nSecond line of it.\n" + "word " * 300)
        bundle = manifest.verify_assets_dir(self.assets())
        plan = cism.plan_cism(validate_cism(data), make_context(core.pymupdf, bundle))
        status, doc, out = self.render(data)
        self.assertEqual(status, 0, doc)
        with open(out, "rb") as handle:
            result = parity.check_parity(core.pymupdf, handle.read(), plan)
        self.assertEqual((result["mismatches"], result["rawRn"], result["failed"]), (0, 0, []))
        self.assertEqual(result["pages"], len(plan.pages))
        code, printed = self.parity(data, out)
        self.assertEqual((code, printed["status"], printed["mismatches"], printed["rawRn"]), (0, "ok", 0, 0))
        self.assertNotIn("Learner", json.dumps(printed))

    def test_r19_parity_counts_a_displaced_field_and_a_raw_rn(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context

        data = synthetic_cism()
        bundle = manifest.verify_assets_dir(self.assets())
        plan = cism.plan_cism(validate_cism(data), make_context(core.pymupdf, bundle))
        _, _, out = self.render(data)
        pdf = core.pymupdf.open(out)
        pdf[0].insert_text((100, 700), "stray rn", fontsize=10)
        result = parity.check_parity(core.pymupdf, pdf.tobytes(), plan)
        self.assertEqual(result["rawRn"], 1)
        self.assertGreaterEqual(result["mismatches"], 1)

    def test_r18_missing_banner_template_refuses_with_asset_missing(self):
        from studyq_linux.render import render_cism

        contents = cism_contents()
        del contents[cism.TEMPLATE_DIR + "5_3.jpg"]
        with self.assertRaises(StudyqError) as caught:
            render_cism(synthetic_cism(), manifest.AssetBundle.from_mapping(contents))
        self.assertEqual(caught.exception.code, "asset_missing")
        render_cism(synthetic_cism(sections=[(4, 1)]), manifest.AssetBundle.from_mapping(contents))


@unittest.skipUnless(core.ASSETS_DIR and core.RENDER_ENABLED, "no assets directory given (STUDYQ_ASSETS_DIR)")
class OfficialTemplates(core.TempDirCase):
    def test_r19_cism_parity_against_the_official_templates(self):
        from studyq_linux import parity
        from studyq_linux.render import make_context, render_cism

        data = validate_cism(synthetic_cism(rows=75, text="Invented narrative.\n" + "word " * 400))
        bundle = manifest.verify_assets_dir(core.ASSETS_DIR)
        pdf = render_cism(data, bundle)
        plan = cism.plan_cism(data, make_context(core.pymupdf, bundle))
        result = parity.check_parity(core.pymupdf, pdf, plan)
        self.assertEqual((result["mismatches"], result["rawRn"], result["pages"]), (0, 0, len(plan.pages)))
        self.assertEqual(result["pages"], 1 + 2 + len(narrative_pages(plan)))


if __name__ == "__main__":
    unittest.main()
