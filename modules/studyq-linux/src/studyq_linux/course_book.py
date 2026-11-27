"""The `course_book` official textbook report: layout as data (specs/studyq-linux/core.md R13-R17, R29).

`plan_course_book` turns a validated input into pages: a template, images, shapes and text
fields with their boxes. It is pure Python; text metrics and image sizes come from the caller.
The drawing is `render.py`, the parity check reads the same field table (`parity.py`).
"""

from dataclasses import dataclass, field

from .geometry import ChartImage, PAGE_H, box_px, centered_rect, px, wedge_points
from .schema import LEVELS
from .textutil import fit_line, format_number, wrap_text

REPORT_TYPE = "course_book"
TEMPLATE_DIR = "image/course_book/"
ROWS_PER_PAGE = 25
CHAPTER_ROW_TOP_PX = 1110.0
CHAPTER_ROW_STEP_PX = 88.0
MARK_X_PT = 503.86
GRADE_X_PT = 35.99
GRADE_Y_HPDF = 745.89
CHART_X_PT = 35.99
PAD_PT = 1.0

COLOR_TEXT = (0.1, 0.1, 0.1)
COLOR_MY = (0.91, 0.45, 0.12)
COLOR_AVG = (0.75, 0.75, 0.75)
LEVEL_COLORS = {
    "top": (0.12, 0.47, 0.71),
    "high": (0.12, 0.63, 0.58),
    "mid": (0.55, 0.73, 0.15),
    "low": (0.96, 0.62, 0.07),
}

SCORE_RANGE = ChartImage(TEMPLATE_DIR + "5p/score_range_chart.jpg", CHART_X_PT, 721.89)
TEST_LIST = ChartImage(TEMPLATE_DIR + "5p/test_list_chart.jpg", CHART_X_PT, 577.90)
MUNIT = ChartImage(TEMPLATE_DIR + "4p/munit_chart.jpg", CHART_X_PT, 697.90)
TEST_COLUMNS_PX = (429.5, 243.0)  # first grid column and step of the test chart
MUNIT_COLUMNS_PX = (348.5, 162.0)  # first grid column and step of the unit chart
SCORE_RANGE_AXIS_PX = (316.0, 16.92, 104.0)  # x at score 0, px per point, marker row

MARK_O = "image/mark_o.jpg"
MARK_X = "image/mark_x.jpg"
STAR = TEMPLATE_DIR + "5p/my_score_point.png"
SQUARE = TEMPLATE_DIR + "5p/avg_score_point.png"
DOT = TEMPLATE_DIR + "4p/my_score_point.png"
MUNIT_BAR_WIDTH_PX = 38.0  # width of the average bar of the unit chart

# Page 4 columns, by the heading of the template: (x0, x1) in template pixels.
CHAPTER_COLUMNS_PX = {
    "no": (212.0, 408.0),  # heading: item number
    "type": (412.0, 1356.0),  # heading: type
    "question_type": (1362.0, 1632.0),  # heading: question type
    "difficulty": (1636.0, 1822.0),  # heading: difficulty
    "correct_rate": (1828.0, 2014.0),  # heading: correct rate (%)
}
CHAPTER_MARK_COLUMN_PX = (2020.0, 2268.0)
TEST_ROW_TOP_PX = 2360.0
TEST_ROW_STEP_PX = 118.5
TEST_COLUMNS_ROW_PX = {
    "no": (212.0, 324.0),
    "title": (330.0, 1436.0),
    "questions": (1444.0, 1630.0),
    "average": (1636.0, 1820.0),
    "score": (1828.0, 2014.0),
    "date": (2020.0, 2268.0),
}
MUNIT_ROW_TOP_PX = 1882.0
MUNIT_ROW_STEP_PX = 118.8
MUNIT_COLUMNS_ROW_PX = {"name": (330.0, 1314.0), "correct": (1320.0, 1568.0), "total": (1574.0, 1822.0)}
MUNIT_BAR_PX = (1836.0, 4.3)  # x at score 0, px per point
LEVEL_ROW_TOPS_PX = (632.0, 802.0, 974.0, 1144.0)
LEVEL_BAR_LEFT_PX = (594.0, 1020.0, 1446.0, 1870.0)
LEVEL_BAR_WIDTH_PX = 134.0
LEVEL_BASE_PX = 3110.0
LEVEL_PX_PER_POINT = 7.6


@dataclass(frozen=True)
class Field:
    id: str
    text: str
    box: tuple  # (x0, y0, x1, y1) in pt, PyMuPDF orientation
    align: str  # "l", "c" or "r"
    size: float


@dataclass(frozen=True)
class ImageOp:
    rel: str
    rect: tuple


@dataclass(frozen=True)
class ShapeOp:
    kind: str  # "rect" or "poly"
    geometry: tuple
    fill: tuple


@dataclass
class PagePlan:
    template: str
    images: list = field(default_factory=list)
    shapes: list = field(default_factory=list)
    fields: list = field(default_factory=list)


@dataclass
class Plan:
    pages: list


@dataclass(frozen=True)
class PlanContext:
    measure: object  # (text, size_pt) -> width_pt
    image_size: object  # (relative path) -> (width_px, height_px)


def template_sequence(chapter_row_counts):
    """The template of every page (R29): 1, 2, 3, then per chapter 4 (5 on overflow), 6, 7."""
    seq = [TEMPLATE_DIR + "coursebook_%d.jpg" % n for n in (1, 2, 3)]
    for rows in chapter_row_counts:
        pages = max(1, -(-rows // ROWS_PER_PAGE))
        seq.append(TEMPLATE_DIR + "coursebook_4.jpg")
        seq.extend([TEMPLATE_DIR + "coursebook_5.jpg"] * (pages - 1))
    seq.extend(TEMPLATE_DIR + "coursebook_%d.jpg" % n for n in (6, 7))
    return seq


def _text(ctx, fid, text, box, align, size):
    if not text:
        return None
    pt = box_px(*box)
    shown, final = fit_line(ctx.measure, text, size, pt[2] - pt[0] - 2 * PAD_PT)
    return Field(fid, shown, pt, align, final)


def _add(page, item):
    if item is not None:
        page.fields.append(item)


def _bar(page, x0_px, x1_px, y0_px, y1_px, color):
    if x1_px > x0_px and y1_px > y0_px:
        page.shapes.append(ShapeOp("rect", box_px(x0_px, y0_px, x1_px, y1_px), color))


def _cover(ctx, data):
    page = PagePlan(TEMPLATE_DIR + "coursebook_1.jpg")
    _add(page, _text(ctx, "p1.period", data["period"], (1780, 2950, 2300, 3050), "l", 14))
    _add(page, _text(ctx, "p1.student", data["student"], (1780, 3136, 2300, 3236), "l", 14))
    return page


def _grade_page(ctx, data):
    page = PagePlan(TEMPLATE_DIR + "coursebook_2.jpg")
    rel = TEMPLATE_DIR + "1p/grade_%02d.jpg" % data["grade"]
    w, h = ctx.image_size(rel)
    top = PAGE_H - GRADE_Y_HPDF
    page.images.append(ImageOp(rel, (GRADE_X_PT, top, GRADE_X_PT + px(w), top + px(h))))
    _add(page, _text(ctx, "p2.my_average", format_number(data["my_average"]), (1500, 640, 2180, 790), "r", 24))
    _add(page, _text(ctx, "p2.all_average", format_number(data["all_average"]), (1500, 900, 2180, 1050), "r", 24))
    x0, per = 322.4, 18.36
    _bar(page, x0, x0 + per * data["my_average"], 1330, 1420, COLOR_MY)
    _bar(page, x0, x0 + per * data["all_average"], 1470, 1560, COLOR_AVG)
    size = 11.0
    line_px = size * 1.6 / px(1)
    top_px, left_px, right_px, bottom_px = 1984.0 + 40, 236.0 + 40, 2204.0 - 40, 3260.0 - 40
    width = px(right_px - left_px) - 2 * PAD_PT
    lines = wrap_text(ctx.measure, data["comment"], size, width) if data["comment"] else []
    capacity = max(1, int((bottom_px - top_px) // line_px))
    if len(lines) > capacity:
        lines = lines[:capacity]
        lines[-1] = fit_line(ctx.measure, lines[-1] + "...", size, width, min_size=size)[0]
    for k, line in enumerate(lines):
        y0 = top_px + k * line_px
        _add(page, _text(ctx, "p2.comment.%d" % k, line, (left_px, y0, right_px, y0 + line_px), "l", size))
    return page


def _marker(ctx, rel, center):
    return ImageOp(rel, centered_rect(center, ctx.image_size(rel)))


def _tests_page(ctx, data):
    page = PagePlan(TEMPLATE_DIR + "coursebook_3.jpg")
    for chart in (SCORE_RANGE, TEST_LIST):
        page.images.append(ImageOp(chart.rel, chart.rect(ctx.image_size(chart.rel))))
    x0, per, row = SCORE_RANGE_AXIS_PX
    for rel, score in ((SQUARE, data["score_range"]["average"]), (STAR, data["score_range"]["my"])):
        center = (SCORE_RANGE.x_pt + px(x0 + per * score), SCORE_RANGE.top_pt + px(row))
        page.images.append(_marker(ctx, rel, center))
    first, step = TEST_COLUMNS_PX
    for i, item in enumerate(data["test_list"]):
        x = first + step * i
        page.images.append(_marker(ctx, SQUARE, TEST_LIST.point(x, item["average"])))
        page.images.append(_marker(ctx, STAR, TEST_LIST.point(x, item["score"])))
        top = TEST_ROW_TOP_PX + TEST_ROW_STEP_PX * i
        values = {
            "no": str(item["no"]),
            "title": item["title"],
            "questions": str(item["questions"]),
            "average": format_number(item["average"]),
            "score": format_number(item["score"]),
            "date": item["date"],
        }
        for name, (x0c, x1c) in TEST_COLUMNS_ROW_PX.items():
            align = "l" if name == "title" else "c"
            _add(page, _text(ctx, "p3.row%d.%s" % (i, name), values[name], (x0c, top, x1c, top + TEST_ROW_STEP_PX), align, 11))
    return page


def _chapter_pages(ctx, index, chapter):
    pages = []
    questions = chapter["questions"]
    chunks = [questions[k : k + ROWS_PER_PAGE] for k in range(0, len(questions), ROWS_PER_PAGE)]
    for n, chunk in enumerate(chunks):
        number = "p4.c%d" % index if n == 0 else "p5.c%d.%d" % (index, n)
        page = PagePlan(TEMPLATE_DIR + ("coursebook_4.jpg" if n == 0 else "coursebook_5.jpg"))
        if n == 0:
            _add(page, _text(ctx, number + ".name", chapter["name"], (760, 60, 2260, 160), "r", 14))
            _add(page, _text(ctx, number + ".my_score", format_number(chapter["my_score"]), (1700, 470, 2210, 590), "r", 22))
            _add(page, _text(ctx, number + ".average", format_number(chapter["average"]), (1700, 725, 2210, 845), "r", 22))
            x0, per = 212.6, 12.53
            _bar(page, x0, x0 + per * chapter["my_score"], 520, 600, COLOR_MY)
            _bar(page, x0, x0 + per * chapter["average"], 640, 720, COLOR_AVG)
        for i, q in enumerate(chunk):
            top = CHAPTER_ROW_TOP_PX + CHAPTER_ROW_STEP_PX * i
            values = {
                "no": str(q["no"]),
                "type": q["type"],
                "question_type": q["question_type"],
                "difficulty": q["difficulty"],
                "correct_rate": format_number(q["correct_rate"]),
            }
            for name, (x0c, x1c) in CHAPTER_COLUMNS_PX.items():
                align = "l" if name == "type" else "c"
                _add(page, _text(ctx, "%s.row%d.%s" % (number, i, name), values[name], (x0c, top, x1c, top + CHAPTER_ROW_STEP_PX), align, 11))
            rel = MARK_O if q["correct"] else MARK_X
            w, h = ctx.image_size(rel)
            y = px(top + (CHAPTER_ROW_STEP_PX - h) / 2)
            page.images.append(ImageOp(rel, (MARK_X_PT, y, MARK_X_PT + px(w), y + px(h))))
        pages.append(page)
    return pages


def _difficulty_page(ctx, data):
    page = PagePlan(TEMPLATE_DIR + "coursebook_6.jpg")
    total = sum(data["difficulty"][lv]["questions"] for lv in LEVELS)
    start = 0.0
    for k, level in enumerate(LEVELS):
        item = data["difficulty"][level]
        ratio = item["questions"] * 100.0 / total if total else 0.0
        top = LEVEL_ROW_TOPS_PX[k]
        text = "%s%% / %d" % (format_number(round(ratio, 1)), item["questions"])
        _add(page, _text(ctx, "p6.%s.share" % level, text, (1440, top + 30, 2160, top + 114), "r", 16))
        if total and item["questions"]:
            end = start + 360.0 * item["questions"] / total
            points = wedge_points(px(760), px(1120), px(300), start, end)
            page.shapes.append(ShapeOp("poly", tuple(points), LEVEL_COLORS[level]))
            start = end
        left = LEVEL_BAR_LEFT_PX[k]
        y = LEVEL_BASE_PX - LEVEL_PX_PER_POINT * item["score"]
        _bar(page, left, left + LEVEL_BAR_WIDTH_PX, y, LEVEL_BASE_PX, LEVEL_COLORS[level])
        _add(page, _text(ctx, "p6.%s.score" % level, format_number(item["score"]), (left, y - 90, left + LEVEL_BAR_WIDTH_PX, y - 10), "c", 14))
    return page


def _munit_page(ctx, data):
    page = PagePlan(TEMPLATE_DIR + "coursebook_7.jpg")
    page.images.append(ImageOp(MUNIT.rel, MUNIT.rect(ctx.image_size(MUNIT.rel))))
    first, step = MUNIT_COLUMNS_PX
    for i, item in enumerate(data["munit_list"]):
        x = first + step * i
        mine = item["correct"] * 100.0 / item["total"]
        base = MUNIT.point(x, 0)
        tip = MUNIT.point(x, item["average"])
        half = px(MUNIT_BAR_WIDTH_PX) / 2
        page.shapes.append(ShapeOp("rect", (base[0] - half, tip[1], base[0] + half, base[1]), COLOR_AVG))
        page.images.append(_marker(ctx, DOT, MUNIT.point(x, mine)))
        top = MUNIT_ROW_TOP_PX + MUNIT_ROW_STEP_PX * i
        for name, (x0c, x1c) in MUNIT_COLUMNS_ROW_PX.items():
            text = item[name] if name == "name" else str(item[name])
            align = "l" if name == "name" else "c"
            _add(page, _text(ctx, "p7.row%d.%s" % (i, name), text, (x0c, top, x1c, top + MUNIT_ROW_STEP_PX), align, 11))
        zero, per = MUNIT_BAR_PX
        _bar(page, zero, zero + per * mine, top + 38, top + 62, COLOR_MY)
    return page


def plan_course_book(data, ctx):
    pages = [_cover(ctx, data), _grade_page(ctx, data), _tests_page(ctx, data)]
    for index, chapter in enumerate(data["bunit_list"]):
        pages.extend(_chapter_pages(ctx, index, chapter))
    pages.append(_difficulty_page(ctx, data))
    pages.append(_munit_page(ctx, data))
    return Plan(pages)


def required_assets(data):
    """Relative paths of every asset a plan for `data` reads."""
    rels = {TEMPLATE_DIR + "coursebook_%d.jpg" % n for n in (1, 2, 3, 4, 6, 7)}
    if any(len(c["questions"]) > ROWS_PER_PAGE for c in data["bunit_list"]):
        rels.add(TEMPLATE_DIR + "coursebook_5.jpg")
    rels.update(
        {
            TEMPLATE_DIR + "1p/grade_%02d.jpg" % data["grade"],
            SCORE_RANGE.rel,
            TEST_LIST.rel,
            MUNIT.rel,
            MARK_O,
            MARK_X,
            STAR,
            SQUARE,
            DOT,
        }
    )
    return sorted(rels)
